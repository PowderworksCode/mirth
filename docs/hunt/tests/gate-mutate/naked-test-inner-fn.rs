fn host<T>() {
    #[test]
    #[unsafe(naked)]
    extern "C" fn t() {}
}
