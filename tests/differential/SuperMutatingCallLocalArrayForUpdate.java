public class SuperMutatingCallLocalArrayForUpdate {
    public static String run() {
        Derived d = new Derived();
        d.addWithUnary(3);
        d.addWithAssignment(2);
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseLocalArrayForUpdate {
    int n;

    void addWithUnary(int value) {
        for (int[] remaining = new int[] { value }; remaining[0] > 0; remaining[0]--) {
            this.n++;
        }
    }

    void addWithAssignment(int value) {
        for (int[] remaining = new int[] { value }; remaining[0] > 0; remaining[0] -= 1) {
            this.n++;
        }
    }
}

class Derived extends BaseLocalArrayForUpdate {
    void addWithUnary(int value) {
        super.addWithUnary(value);
        this.n += 10;
    }

    void addWithAssignment(int value) {
        super.addWithAssignment(value);
        this.n += 10;
    }
}
