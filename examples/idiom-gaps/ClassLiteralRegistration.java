class ClassLiteralRegistration {
    void register(Kryo kryo) {
        kryo.register(RegisteredType.class);
    }
}

