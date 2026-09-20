interface BranchMeasure {
    int measure(boolean enabled);
}

public class AnonymousInterfaceLocalBranch {
    static BranchMeasure make(int bonus) {
        return new BranchMeasure() {
            int base = 1;

            public int measure(boolean enabled) {
                int local = this.base;
                if (enabled) {
                    int bonus = 7;
                    local = bonus;
                } else {
                    local = local + bonus;
                }
                return local;
            }
        };
    }

    public static void main(String[] args) {
        BranchMeasure value = make(5);
        System.out.println(value.measure(false));
        System.out.println(value.measure(true));
    }
}
