pub struct Logger {}

impl Logger {
    pub fn init() {
        let mut builder = env_logger::Builder::new();
        builder.filter_level(log::LevelFilter::Debug); // TODO: allow customization
        builder.init();
    }
}
