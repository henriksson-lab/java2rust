package implicitcollectionaction;
public class ImplicitCollectionActionAudit {
    int total;
    void add(String value) { total += value.length(); }
    public static int run() {
        java.util.Set<String> values = new java.util.HashSet<>();
        values.add("abc"); values.add("de");
        ImplicitCollectionActionAudit target = new ImplicitCollectionActionAudit();
        values.forEach(target::add);
        values.forEach(value -> target.add(value));
        return target.total == 10 ? 0 : 1;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
