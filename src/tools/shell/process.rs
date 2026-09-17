use std::{process::Stdio, time::Duration};

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    time::timeout,
};

use crate::error::ToolInputError;

use super::ShellOutput;

pub(super) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
pub(super) const DEFAULT_MAX_OUTPUT_BYTES: usize = 8_000;

/// 子进程退出后再给读取端一点时间，冲掉管道里剩余的数据。
///
/// 没有它，`cmd &` 这类把 stdout 留给孙进程的命令永远等不到 EOF，
/// 只能耗满整个 timeout 才返回。
const DRAIN_GRACE: Duration = Duration::from_millis(200);

/// SIGKILL 之后通常立刻收尸，但处于不可中断睡眠的进程会推迟退出，
/// 所以这次等待必须有上界；即使放弃等待也不会留下僵尸进程，
/// tokio 会把已杀死但未回收的子进程交给内部 reaper 处理。
const KILL_GRACE: Duration = Duration::from_secs(2);

#[derive(Clone, Copy)]
struct Limits {
    timeout: Duration,
    max_output_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        }
    }
}

/// 单路输出的有界缓冲。
///
/// 所有权留在 [`run_with`] 里，读取端只借用它，这样即使超时或会话取消把 future 丢掉，
/// 已经读到的内容也不会跟着消失。
#[derive(Default)]
struct Capped {
    bytes: Vec<u8>,
    truncated: bool,
    failed: bool,
}

impl Capped {
    fn push(&mut self, chunk: &[u8], limit: usize) {
        let room = limit.saturating_sub(self.bytes.len());
        if room == 0 {
            self.truncated = true;
            return;
        }
        let take = room.min(chunk.len());
        self.bytes.extend_from_slice(&chunk[..take]);
        if take < chunk.len() {
            self.truncated = true;
        }
    }
}

/// 读取一路输出，超过 `limit` 后继续排空管道但不再保存，
/// 否则子进程会因为管道缓冲写满而阻塞在写操作上。
async fn read_capped<R>(mut reader: R, sink: &mut Capped, limit: usize)
where
    R: AsyncRead + Unpin,
{
    let mut chunk = [0u8; 8192];
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) => return,
            Ok(read) => sink.push(&chunk[..read], limit),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                sink.failed = true;
                return;
            }
        }
    }
}

pub(super) async fn run(command: &str) -> Result<ShellOutput, ToolInputError> {
    run_with(command, Limits::default()).await
}

async fn run_with(command: &str, limits: Limits) -> Result<ShellOutput, ToolInputError> {
    let working_directory = std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "未知".to_owned());

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // 会话取消或 worker 丢弃 reply future 时兜底，命令不会变成孤儿进程。
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| ToolInputError("无法启动 shell 子进程"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or(ToolInputError("stdout 未按 piped 建立"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or(ToolInputError("stderr 未按 piped 建立"))?;
    let max = limits.max_output_bytes;
    let mut out = Capped::default();
    let mut err = Capped::default();

    // 子进程退出和两路输出读取必须并发进行：只等其中一边，另一边写满管道缓冲后
    // 子进程就会阻塞，双方互相等待。块作用域让这里所有的借用先于下面结束。
    let outcome = {
        let execution = async {
            let readers = async {
                tokio::join!(
                    read_capped(stdout, &mut out, max),
                    read_capped(stderr, &mut err, max),
                );
            };
            tokio::pin!(readers);

            tokio::select! {
                status = child.wait() => {
                    let _ = timeout(DRAIN_GRACE, &mut readers).await;
                    status
                }
                // 两路都到 EOF 时子进程不一定已经退出，改为等它结束，由外层超时兜底。
                _ = &mut readers => child.wait().await,
            }
        };
        timeout(limits.timeout, execution).await
    };

    let (exit_code, timed_out) = match outcome {
        Ok(status) => (
            status
                .map_err(|_| ToolInputError("等待 shell 子进程失败"))?
                .code(),
            false,
        ),
        Err(_) => {
            // kill_on_drop 只在 drop 时兜底，这里主动发信号并限时收尸。
            let _ = child.start_kill();
            let _ = timeout(KILL_GRACE, child.wait()).await;
            (None, true)
        }
    };

    if out.failed || err.failed {
        return Err(ToolInputError("读取 shell 子进程输出失败"));
    }

    Ok(ShellOutput {
        command: command.to_owned(),
        working_directory,
        exit_code,
        stdout: String::from_utf8_lossy(&out.bytes).into_owned(),
        stderr: String::from_utf8_lossy(&err.bytes).into_owned(),
        truncated: out.truncated || err.truncated,
        timed_out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits_with_timeout(timeout: Duration) -> Limits {
        Limits {
            timeout,
            ..Limits::default()
        }
    }

    #[tokio::test]
    async fn captures_stdout_and_exit_code() {
        let output = run("printf 'hello'").await.unwrap();
        assert_eq!(output.stdout, "hello");
        assert_eq!(output.stderr, "");
        assert_eq!(output.exit_code, Some(0));
        assert!(!output.truncated);
        assert!(!output.timed_out);
    }

    #[tokio::test]
    async fn reports_non_zero_exit_code_as_data() {
        let output = run("exit 3").await.unwrap();
        assert_eq!(output.exit_code, Some(3));
        assert_eq!(output.stdout, "");
    }

    #[tokio::test]
    async fn captures_stderr_separately() {
        let output = run("printf 'oops' 1>&2").await.unwrap();
        assert_eq!(output.stdout, "");
        assert_eq!(output.stderr, "oops");
    }

    #[tokio::test]
    async fn truncates_output_beyond_the_limit_without_deadlocking() {
        // 200000 字节远超管道缓冲，验证超出上限后仍会排空，子进程不会写阻塞。
        let limits = Limits {
            max_output_bytes: 64,
            ..Limits::default()
        };
        let output = run_with("head -c 200000 /dev/zero | tr '\\0' a", limits)
            .await
            .unwrap();
        assert!(output.truncated);
        assert_eq!(output.stdout.len(), 64);
        assert_eq!(output.exit_code, Some(0));
    }

    #[tokio::test]
    async fn times_out_and_keeps_partial_output() {
        let output = run_with(
            "printf 'partial'; sleep 5",
            limits_with_timeout(Duration::from_millis(300)),
        )
        .await
        .unwrap();
        assert!(output.timed_out);
        assert_eq!(output.exit_code, None);
        assert_eq!(output.stdout, "partial");
    }

    #[tokio::test]
    async fn returns_promptly_when_a_grandchild_holds_the_pipe() {
        // `sleep 3 &` 让 sh 立刻退出，但孙进程仍持有 stdout 写端。
        let started = std::time::Instant::now();
        let output = run("sleep 3 & printf 'started'").await.unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "已退出命令不应等到管道 EOF：{:?}",
            started.elapsed()
        );
        assert_eq!(output.stdout, "started");
        assert!(!output.timed_out);
        assert_eq!(output.exit_code, Some(0));
    }

    #[tokio::test]
    async fn dropping_the_execution_future_kills_a_running_command() {
        // worker.rs 在取消或收到新消息时会丢掉整个 reply future，这里走同一条路径。
        let marker = format!("sleep 61.{:04}", std::process::id() % 10_000);
        {
            let execution = run(&marker);
            tokio::pin!(execution);
            // 先让它跑到子进程真正启动，再丢弃 future。
            let _ = timeout(Duration::from_millis(300), &mut execution).await;
        }

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while process_exists(&marker) && std::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(!process_exists(&marker), "取消后命令仍在运行：{marker}");
    }

    fn process_exists(marker: &str) -> bool {
        std::process::Command::new("pgrep")
            .args(["-f", marker])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[tokio::test]
    async fn runs_in_the_process_working_directory() {
        let expected = std::env::current_dir().unwrap().display().to_string();
        let output = run("pwd").await.unwrap();
        assert_eq!(output.working_directory, expected);
        assert_eq!(output.stdout.trim(), expected);
    }
}
