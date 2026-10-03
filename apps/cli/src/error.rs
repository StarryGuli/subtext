use subtext_dictionary::DictionaryError;
use subtext_learning::LearningError;
use subtext_lm::LmError;
use subtext_neural::NeuralError;
use subtext_platform::ConfigError;
use subtext_predict::PredictError;
use subtext_translate::GlossaryError;
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    Cold(#[from] crate::cold::error::ColdError),

    #[error(transparent)]
    Dictionary(#[from] DictionaryError),

    #[error(transparent)]
    Neural(#[from] NeuralError),

    #[error(transparent)]
    Glossary(#[from] GlossaryError),

    #[error(transparent)]
    Learning(#[from] LearningError),

    /// 学习语言不是 en / ja / es。
    #[error("learning language must be en, ja or es, got {0:?}")]
    Language(String),

    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Predict(#[from] PredictError),

    #[error(transparent)]
    LanguageModel(#[from] LmError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Replay(#[from] crate::replay::ReplayError),

    #[error(transparent)]
    Eval(#[from] crate::eval::EvalError),

    #[error(transparent)]
    Tune(#[from] crate::tuning::TuneError),
}
