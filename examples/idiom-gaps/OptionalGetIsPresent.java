class OptionalGetIsPresent {
    String unwrap(Optional<String> value) {
        if (value.isPresent()) {
            return value.get();
        }
        return "missing";
    }
}

