public class NullFieldCtorOverload {
    static class Holder {
        String name;
        Holder() { }
        Holder(String n) { this.name = n; }
        boolean empty() { return name == null; }
    }
    public static String run() {
        return new Holder().empty() + "," + new Holder("a").empty();
    }
    public static void main(String[] args) { System.out.println(run()); }
}
