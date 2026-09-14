public class QualifiedInnerCapture {
    private int base = 10;

    class Inner {
        int get() {
            return base;
        }
    }

    int compute() {
        QualifiedInnerCapture other = new QualifiedInnerCapture();
        other.base = 20;
        Inner inner = other.new Inner();
        other.base = 21;
        return inner.get();
    }

    public static String run() {
        return String.valueOf(new QualifiedInnerCapture().compute());
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
