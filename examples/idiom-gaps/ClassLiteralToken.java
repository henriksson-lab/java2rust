class ClassLiteralToken {
    Class<?> stringToken() {
        return String.class;
    }

    boolean matches(Object value) {
        return value.getClass().equals(String.class);
    }
}
