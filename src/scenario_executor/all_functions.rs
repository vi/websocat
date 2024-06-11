use rhai::Engine;

pub(crate) fn register_functions(engine: &mut Engine) {
    crate::scenario_executor::trivials::register(engine);
    crate::scenario_executor::copydata::register(engine);
    crate::scenario_executor::misc::register(engine);
    crate::scenario_executor::tcp::register(engine);
    crate::scenario_executor::fluff::register(engine);
    crate::scenario_executor::wsupgrade::register(engine);
}
