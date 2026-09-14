public class SuperMutatingCallSwitch {
    public static String run() {
        Derived d = new Derived();
        d.add(1);
        d.add(2);
        d.add(3);
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseSwitch {
    int n;

    void add(int value) {
        switch (value) {
            case 1:
                this.n += 1;
                break;
            case 2:
                this.n += 2;
            default:
                this.n += 3;
                break;
        }
    }
}

class Derived extends BaseSwitch {
    void add(int value) {
        super.add(value);
        this.n += 10;
    }
}
