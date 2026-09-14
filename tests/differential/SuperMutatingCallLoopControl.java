public class SuperMutatingCallLoopControl {
    public static String run() {
        Derived d = new Derived();
        d.addWithBreak(4);
        d.addWithContinue(3);
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseLoopControl {
    int n;

    void addWithBreak(int value) {
        while (value > 0) {
            if (value == 2) {
                break;
            }
            this.n++;
            value--;
        }
    }

    void addWithContinue(int value) {
        while (value > 0) {
            value--;
            if (value == 1) {
                continue;
            }
            this.n++;
        }
    }
}

class Derived extends BaseLoopControl {
    void addWithBreak(int value) {
        super.addWithBreak(value);
        this.n += 10;
    }

    void addWithContinue(int value) {
        super.addWithContinue(value);
        this.n += 10;
    }
}
