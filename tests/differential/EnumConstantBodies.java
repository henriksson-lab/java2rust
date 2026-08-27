public class EnumConstantBodies {
    enum Op {
        ADD {
            int apply(int a, int b) {
                return a + b;
            }
        },
        MUL {
            int apply(int a, int b) {
                return a * b;
            }
        };

        abstract int apply(int a, int b);
    }

    public static String run() {
        return Op.ADD.apply(2, 3) + "," + Op.MUL.apply(2, 3);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
