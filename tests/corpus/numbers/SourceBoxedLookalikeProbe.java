class SourceBoxedLookalikeProbe {
    static class Double {
        int calls;
        public boolean equals(Object other) { calls++; return false; }
    }
    static int run() {
        Double source = new Double();
        if (source.equals(source) || source.equals(null) || source.calls != 2) return 1;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
