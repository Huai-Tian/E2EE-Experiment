//! E2EE-Experiment 库 —— 一个像人一样的端到端加密对话内核。
//!
//! 本 crate 既是命令行程序，也是库（crate 名 `e2ee`）：
//! - 内核在这里：**不打印任何东西、不读任何输入**——
//!   动作从方法进（`Person::born` / `dial` / `speak` / `talk_to` / `card` /
//!   `shout` / `bye` / `leave`），经历从事件出（[`Event`] 流）；
//! - `src/main.rs` 只是第一号使用者：把事件渲染成终端文字。
//!
//! 供其他程序接入（Windows / Linux 均可）：
//! - Rust 程序直接依赖本 crate；
//! - 任何能调 C 的语言经 `ffi/` 门面 crate（`.so` / `.dll`，头文件
//!   `include/e2ee.h`，契约测试 `ffi/ffi_test.c`）。

pub mod circle;
pub mod courier;
pub mod discover;
pub mod person;
pub mod punch;
pub mod wire;

pub use discover::Discovered;
pub use person::{Event, Events, LeaveReason, Person, PersonError, RosterEntry};
