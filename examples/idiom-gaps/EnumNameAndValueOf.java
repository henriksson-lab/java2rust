class EnumNameAndValueOf {
    String roundTrip(String text) {
        Mode mode = Mode.valueOf(text);
        return mode.name();
    }
}

