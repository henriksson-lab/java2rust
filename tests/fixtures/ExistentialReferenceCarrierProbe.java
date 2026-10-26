interface CapturedValue<T> {
    int value();
    void add(int amount);
}
class CapturedText implements CapturedValue<String> {
    int count = 3;
    public int value() { return count; }
    public void add(int amount) { count += amount; }
}
class CapturedNumber implements CapturedValue<Integer> {
    int count = 11;
    public int value() { return count; }
    public void add(int amount) { count += amount; }
}
class ExistentialReferenceCarrierProbe {
    static CapturedValue<?> choose(boolean text) {
        if (text) return new CapturedText();
        return new CapturedNumber();
    }
    static <T> int update(CapturedValue<T> value) {
        value.add(2);
        return value.value();
    }
    static int run() {
        CapturedValue<?> text = choose(true);
        CapturedValue<?> alias = text;
        if (text != alias) return 1;
        if (update(text) != 5 || update(alias) != 7) return 2;
        if (update(choose(false)) != 13) return 3;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
