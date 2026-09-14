public class InnerCapture {
    private int base = 10;

    class Adder {
        int add(int x) {
            return base + x;
        }
    }

    int compute() {
        Adder a = new Adder();
        base = 20;
        return a.add(1);
    }

    public static String run() {
        return String.valueOf(new InnerCapture().compute());
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
