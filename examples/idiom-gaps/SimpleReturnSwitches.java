class SimpleReturnSwitches {
    String numberLabel(int value) {
        switch (value) {
            case 1:
                return "one";
            case 2:
            case 3:
                return "small";
            default:
                return "other";
        }
    }

    int code(String name) {
        switch (name) {
            case "alpha":
                return 1;
            case "beta":
                return 2;
            default:
                return 0;
        }
    }
}
