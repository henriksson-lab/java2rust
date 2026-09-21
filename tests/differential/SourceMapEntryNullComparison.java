public class SourceMapEntryNullComparison {
    static class Entry implements java.util.Map.Entry<String, Integer> {
        private String key;
        private Integer value;

        Entry(String key, Integer value) { this.key = key; this.value = value; }
        public String getKey() { return key; }
        public Integer getValue() { return value; }
        public Integer setValue(Integer value) {
            Integer old = this.value;
            this.value = value;
            return old;
        }
    }

    public static String run() {
        java.util.Map.Entry<String, Integer> entry = new Entry(null, null);
        boolean nullKey = entry.getKey() == null;
        boolean nullValue = entry.getValue() == null;
        boolean oldWasNull = entry.setValue(4) == null;
        boolean presentValue = entry.getValue() != null;
        boolean oldWasPresent = entry.setValue(null) != null;
        boolean nullAgain = entry.getValue() == null;
        return nullKey + ":" + nullValue + ":" + oldWasNull + ":" + presentValue
                + ":" + oldWasPresent + ":" + nullAgain;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
