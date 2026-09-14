public class SuperMutatingCallForAssignmentUpdate {
    public static String run() {
        Derived d = new Derived();
        d.addWithLocalUpdate(3);
        d.addWithParamUpdate(2);
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseForAssignmentUpdate {
    int n;

    void addWithLocalUpdate(int value) {
        for (int i = 0; i < value; i += 1) {
            this.n++;
        }
    }

    void addWithParamUpdate(int value) {
        for (; value > 0; value -= 1) {
            this.n++;
        }
    }
}

class Derived extends BaseForAssignmentUpdate {
    void addWithLocalUpdate(int value) {
        super.addWithLocalUpdate(value);
        this.n += 10;
    }

    void addWithParamUpdate(int value) {
        super.addWithParamUpdate(value);
        this.n += 10;
    }
}
