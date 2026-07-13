class NonNullFieldViaHelper {
    private String name;

    NonNullFieldViaHelper() {
        initializeName();
    }

    private void initializeName() {
        this.name = "default";
    }

    String current() {
        return name;
    }
}
