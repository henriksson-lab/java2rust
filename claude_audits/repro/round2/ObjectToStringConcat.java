public class ObjectToStringConcat {
    static class Point {
        int x;
        Point(int x) { this.x = x; }
        public String toString() { return "P" + x; }
    }
    public static String run() {
        Object o = new Point(3);
        Point p = new Point(4);
        return "a" + o + "," + "b" + p + "," + p.toString() + "," + o.toString();
    }
    public static void main(String[] args) { System.out.println(run()); }
}
