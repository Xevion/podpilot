use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Log formatter to use
    #[arg(long, value_enum, default_value_t = default_tracing_format())]
    pub tracing: TracingFormat,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum TracingFormat {
    /// Use pretty formatter (default in debug mode)
    Pretty,
    /// Use JSON formatter (default in release mode)
    Json,
}

#[cfg(debug_assertions)]
const DEFAULT_TRACING_FORMAT: TracingFormat = TracingFormat::Pretty;
#[cfg(not(debug_assertions))]
const DEFAULT_TRACING_FORMAT: TracingFormat = TracingFormat::Json;

fn default_tracing_format() -> TracingFormat {
    DEFAULT_TRACING_FORMAT
}
