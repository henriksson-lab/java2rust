class ThrowableAppendProbe {
    static int run() {
        Exception error = new IllegalArgumentException("\uD800");
        String expected = "java.lang.IllegalArgumentException: \uD800";
        if (!error.toString().equals(expected)) return 1;
        if (!("error=" + error).equals("error=" + expected)) return 2;
        StringBuilder builder = new StringBuilder();
        builder.append(error);
        return builder.toString().equals(expected) ? 0 : 3;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
