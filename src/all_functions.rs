use rhai::Engine;

pub(crate) fn register_functions(engine: &mut Engine) {
    crate::trivials::register(engine);
    crate::copydata::register(engine);
    crate::misc::register(engine);
    crate::tcp::register(engine);
    crate::fluff::register(engine);
}
