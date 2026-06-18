fn load(payload: Result<(), ()>) {
    payload.expect("loaded");
}
