/** Native reflective text must have one logical ABI in every expression context. */
public class ClassNameProbe {
    String name;
    ClassNameProbe(String name) { this.name = name; }
    public String toString() { return getClass().getName(); }
    static String direct() { return new ClassNameProbe("unused").getClass().getName(); }
    static String relay(String value) { return value; }
    // A source-owned getName must not be mistaken for the reflective native ABI.
    String getName() { return name; }
    public static int run() {
        ClassNameProbe value = new ClassNameProbe("source");
        if (!value.toString().equals("ClassNameProbe")) return 1;
        if (!direct().equals("ClassNameProbe")) return 2;
        String local = value.getClass().getName();
        if (!local.equals("ClassNameProbe")) return 3;
        if (!relay(value.getClass().getName()).equals(local)) return 4;
        ClassNameProbe stored = new ClassNameProbe(value.getClass().getName());
        if (!stored.getName().equals(local)) return 5;
        if (!value.getName().equals("source")) return 6;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
