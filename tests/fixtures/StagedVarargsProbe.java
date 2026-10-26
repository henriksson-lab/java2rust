class StagedVarargsProbe {
    int events;
    String keyword(boolean present) {
        events = events * 10 + 1;
        return present ? "word" : null;
    }
    static int words(int prior, String... words) {
        if (prior != 0) return 10;
        if (words.length == 0) return 0;
        if (words.length == 1) return words[0] == null ? 2 : words[0].length();
        if (words.length == 2 && words[0].equals("word") && words[1] == null) return 6;
        return 11;
    }
    int readEmpty() { return words(events); }
    int readExplicit() { return words(events, new String[] { "word" }); }
    int readNullArray() { return words(events, (String[]) null); }
    int readOne(boolean present) { return words(events, keyword(present)); }
    int readTwo() { return words(events, keyword(true), keyword(false)); }
    static int run() {
        StagedVarargsProbe owner = new StagedVarargsProbe();
        StagedVarargsProbe alias = owner;
        if (owner.readOne(true) != 4 || alias.events != 1) return 1;
        owner.events = 0;
        if (owner.readOne(false) != 2 || alias.events != 1) return 2;
        owner.events = 0;
        if (owner.readTwo() != 6 || alias.events != 11) return 3;
        owner.events = 0;
        if (owner.readEmpty() != 0 || owner.readExplicit() != 4 || alias.events != 0) return 4;
        try { owner.readNullArray(); return 5; }
        catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
