class NonNullFieldFromConstructors {
    private String name;

    NonNullFieldFromConstructors() {
        this.name = "default";
    }

    NonNullFieldFromConstructors(String name) {
        this.name = name;
    }

    void rename(String next) {
        this.name = next;
    }

    String current() {
        return this.name;
    }
}
