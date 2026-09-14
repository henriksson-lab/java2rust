public class SuperMutatingCallEnhancedFor {
    public static String run() {
        Derived d = new Derived();
        d.add(new int[] {1, 2, 3});
        d.add(new int[] {4});
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseEnhancedFor {
    int n;

    void add(int[] values) {
        for (int value : values) {
            this.n += value;
        }
    }
}

class Derived extends BaseEnhancedFor {
    void add(int[] values) {
        super.add(values);
        this.n += 10;
    }
}
