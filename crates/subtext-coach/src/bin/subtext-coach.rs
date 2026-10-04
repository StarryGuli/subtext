//! 命令行试用教练：不经过输入法，直接对一段文本跑一次，看后端通不通、答得怎么样。
//!
//! `subtext-coach "hey, no rush but did you look at the PR?"`：按文本语言自动选解码 / 组句 / 改稿。

use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, ValueEnum};
use subtext_coach::{
    BackendKind, CoachConfig, CoachContext, CoachEvent, CoachOutput, CoachRequest, CoachService,
    Mode, Trigger, test_connection,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Backend {
    ClaudeCli,
    CodexCli,
    Openai,
    Anthropic,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ModeArg {
    Decode,
    Compose,
    Edit,
    Screen,
}

#[derive(Debug, Parser)]
#[command(name = "subtext-coach", about = "言外双语教练：命令行试用")]
struct Args {
    /// 要处理的文本；省略则只做连接测试
    text: Vec<String>,

    #[arg(long, value_enum, default_value = "claude-cli")]
    backend: Backend,

    /// 强制模式；缺省按文本语言判断（英文 → 解码，中文 → 组句）
    #[arg(long, value_enum)]
    mode: Option<ModeArg>,

    /// 模型名，按后端解释
    #[arg(long)]
    model: Option<String>,

    /// 路径：claude / codex 的可执行文件
    #[arg(long)]
    binary: Option<String>,

    /// OpenAI 兼容接口地址
    #[arg(long)]
    base_url: Option<String>,

    /// 当前应用（bundle id），用来推断语域
    #[arg(long)]
    app: Option<String>,

    /// 对方刚发来的消息，写回复时按它的语气接
    #[arg(long)]
    peer: Option<String>,

    /// 超时秒数
    #[arg(long, default_value_t = 90)]
    timeout: u64,

    /// 只打印会发给模型的提示词（系统 + 用户），不发请求；调提示词用
    #[arg(long)]
    print_prompt: bool,

    /// 不发请求，把这个文件当作模型回复来解析并显示；调提示词、验解析用
    #[arg(long, value_name = "文件")]
    from_reply: Option<String>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let config = config_from(&args);
    if args.text.is_empty() {
        return match test_connection(&config) {
            Ok(report) => {
                println!("连接正常：{}（{} ms）", report.backend, report.elapsed_ms);
                ExitCode::SUCCESS
            }
            Err(error) => fail(&subtext_coach::friendly(&error)),
        };
    }
    let text = args.text.join(" ");
    let mode = match args.mode {
        Some(ModeArg::Decode) => Mode::Decode,
        Some(ModeArg::Compose) => Mode::Compose,
        Some(ModeArg::Edit) => Mode::Edit,
        Some(ModeArg::Screen) => Mode::Screen,
        None => match Trigger::ClipboardCopy
            .mode_for(&text)
            .or_else(|| Trigger::ChineseCommitted.mode_for(&text))
        {
            Some(mode) => mode,
            None => return fail("看不出这段文本该走哪个模式，请用 --mode 指定。"),
        },
    };
    let request = CoachRequest {
        id: 1,
        mode,
        text,
        context: CoachContext {
            app: args.app,
            before: String::new(),
            peer_message: args.peer,
        },
    };
    if args.print_prompt {
        let prompt = subtext_coach::prompt::build(&request, &config.profile);
        println!(
            "===== SYSTEM =====\n{}\n\n===== USER =====\n{}",
            prompt.system, prompt.user
        );
        return ExitCode::SUCCESS;
    }
    if let Some(path) = &args.from_reply {
        let reply = match std::fs::read_to_string(path) {
            Ok(reply) => reply,
            Err(error) => return fail(&format!("读不了 {path}：{error}")),
        };
        return match CoachOutput::parse(mode, &reply) {
            Ok(output) => {
                print_output(&output);
                ExitCode::SUCCESS
            }
            Err(error) => fail(&format!("解析失败：{error}")),
        };
    }
    let service = CoachService::start(&config);
    let started = Instant::now();
    if service.submit(request).is_err() {
        return fail("教练线程没有起来。");
    }
    let mut first_partial = false;
    loop {
        for event in service.poll() {
            match event {
                CoachEvent::Started { .. } => {}
                CoachEvent::Partial { .. } => {
                    if !first_partial {
                        first_partial = true;
                        eprintln!("（首批内容 {} ms）", started.elapsed().as_millis());
                    }
                }
                CoachEvent::Finished { output, .. } => {
                    print_output(&output);
                    eprintln!("\n（{} ms）", started.elapsed().as_millis());
                    return ExitCode::SUCCESS;
                }
                CoachEvent::Failed { message, .. } => return fail(&message),
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn config_from(args: &Args) -> CoachConfig {
    let mut config = CoachConfig {
        enabled: true,
        timeout_ms: args.timeout * 1000,
        backend: match args.backend {
            Backend::ClaudeCli => BackendKind::ClaudeCli,
            Backend::CodexCli => BackendKind::CodexCli,
            Backend::Openai => BackendKind::OpenAi,
            Backend::Anthropic => BackendKind::Anthropic,
        },
        ..CoachConfig::default()
    };
    if let Some(model) = &args.model {
        match args.backend {
            Backend::ClaudeCli => config.claude_model = model.clone(),
            Backend::CodexCli => config.codex_model = model.clone(),
            Backend::Openai => config.openai_model = model.clone(),
            Backend::Anthropic => config.anthropic_model = model.clone(),
        }
    }
    if let Some(binary) = &args.binary {
        config.claude_path = binary.clone();
        config.codex_path = binary.clone();
    }
    if let Some(url) = &args.base_url {
        config.openai_base_url = url.clone();
    }
    config
}

fn fail(message: &str) -> ExitCode {
    eprintln!("{message}");
    ExitCode::FAILURE
}

fn print_output(output: &CoachOutput) {
    match output {
        CoachOutput::Decode(decoded) => {
            println!("【解码】{}", decoded.situation);
            for point in &decoded.points {
                println!("\n  {}  [{}]", point.phrase, point.kind);
                println!("    字面：{}", point.literal);
                println!("    实际：{}", point.meaning);
                println!("    中文：{}", point.zh);
                println!("    你可以说：{}", point.usage);
            }
            println!(
                "\n  语气：{} —— {}",
                decoded.tone.register, decoded.tone.subtext
            );
            if !decoded.tone.contrast.is_empty() {
                println!("  对照：{}", decoded.tone.contrast);
            }
            if !decoded.question.is_empty() {
                println!("\n  想一想：{}", decoded.question);
            }
            println!("\n  译文（先自己猜）：{}", decoded.translation);
        }
        CoachOutput::Compose(composed) => {
            println!("【组句】{}", composed.context);
            for option in &composed.options {
                let star = if option.recommended { "★" } else { " " };
                println!("\n {star} [{}] {}", option.register, option.text);
            }
            for point in &composed.points {
                println!(
                    "\n  {} → {}  ({})\n    {}",
                    point.zh, point.en, point.mapping, point.why
                );
            }
            for trap in &composed.traps {
                println!("\n  ⚠ {trap}");
            }
        }
        CoachOutput::Plain { text, .. } => {
            println!("【原文】（模型没有按格式回复）\n{text}");
        }
        CoachOutput::Screen(screened) => {
            println!("【屏幕阅读】");
            for item in &screened.items {
                println!("\n  {}. {}", item.i, item.translation);
                if !item.note.is_empty() {
                    println!("     ↳ {}", item.note);
                }
            }
        }
        CoachOutput::Edit(edited) => {
            println!("【改稿】{}", edited.corrected);
            if !edited.translation.is_empty() {
                println!("  意思：{}", edited.translation);
            }
            for alternative in &edited.alternatives {
                println!("\n  更地道：{}\n    {}", alternative.text, alternative.why);
            }
            for fix in &edited.fixes {
                println!(
                    "\n  {} → {}  [{}]\n    {}",
                    fix.from, fix.to, fix.kind, fix.why
                );
            }
            for kept in &edited.kept {
                println!("\n  ✓ {kept}");
            }
            if !edited.pattern.is_empty() {
                println!("\n  模式：{}", edited.pattern);
            }
        }
    }
}
