interface LoopMeasure {
    int whileMeasure(int limit);
    int doMeasure(int limit);
    int forMeasure();
    int whileBreak();
    int forContinue();
    int forShadow();
}

public class AnonymousInterfaceLocalLoops {
    static LoopMeasure make(int bonus) {
        return new LoopMeasure() {
            int base = 1;

            public int whileMeasure(int limit) {
                int local = this.base;
                while (local < limit) { local = local + bonus; }
                return local;
            }

            public int doMeasure(int limit) {
                int local = this.base;
                do { local = local + bonus; } while (local < limit);
                return local;
            }

            public int forMeasure() {
                int local = this.base;
                for (int step = 0; step < 2; step++) { local = local + bonus; }
                return local;
            }

            public int whileBreak() {
                int local = this.base;
                while (local < 10) { local = local + bonus; if (local >= 3) { break; } }
                return local;
            }

            public int forContinue() {
                int local = this.base;
                for (int step = 0; step < 3; step++) {
                    if (step == 1) { continue; }
                    local = local + bonus;
                }
                return local;
            }

            public int forShadow() {
                int local = this.base;
                for (int bonus = 0; bonus < 2; bonus++) { local = local + bonus; }
                return local;
            }
        };
    }

    public static void main(String[] args) {
        LoopMeasure value = make(2);
        System.out.println(value.whileMeasure(5));
        System.out.println(value.doMeasure(1));
        System.out.println(value.forMeasure());
        System.out.println(value.whileBreak());
        System.out.println(value.forContinue());
        System.out.println(value.forShadow());
    }
}
