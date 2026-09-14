public class SuperMutatingCallArrayAccess {
    public static String run() {
        Derived d = new Derived();
        d.add(new int[] {2, 4, 6});
        d.add(new int[] {1});
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseArrayAccess {
    int n;

    void add(int[] values) {
        for (int i = 0; i < values.length; i++) {
            this.n += values[i];
        }
    }
}

class Derived extends BaseArrayAccess {
    void add(int[] values) {
        super.add(values);
        this.n += 10;
    }
}
