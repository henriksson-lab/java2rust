class NumericTemporaryTextProbe {
    String text;
    int events;
    String get() { events = events * 10 + 1; return text; }
    int radix() { events = events * 10 + 2; return 10; }
    int parse() { return Integer.parseInt(get(), radix()); }
    static int run() {
        NumericTemporaryTextProbe owner = new NumericTemporaryTextProbe();
        owner.text = "42";
        if (owner.parse() != 42 || owner.events != 12) return 1;
        owner.text = null;
        owner.events = 0;
        try { owner.parse(); return 2; }
        catch (NumberFormatException expected) {}
        if (owner.events != 12) return 3;
        owner.text = "bad";
        owner.events = 0;
        try { owner.parse(); return 4; }
        catch (NumberFormatException expected) {}
        if (owner.events != 12) return 5;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
