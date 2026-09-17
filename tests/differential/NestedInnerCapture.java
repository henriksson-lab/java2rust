public class NestedInnerCapture {
    private int base;

    NestedInnerCapture(int base) {
        this.base = base;
    }

    int offset() {
        return base + 1;
    }

    int bump() {
        base++;
        return base;
    }

    class Middle {
        class Deep {
            int value() {
                bump();
                return base + offset();
            }
        }

        int makeDeep() {
            return new Deep().value();
        }
    }

    int runInner() {
        Middle middle = new Middle();
        base = 20;
        return middle.makeDeep();
    }

    public static String run() {
        return String.valueOf(new NestedInnerCapture(10).runInner());
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
