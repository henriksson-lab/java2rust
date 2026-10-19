interface TextLookup {
    String fetch(int source, int target);
}

abstract class RankedLookup implements TextLookup {
    int fetchWeight(int source, int target) {
        return fetch(source, target).length();
    }
    String forward(int source, int target) {
        return fetch(source, target);
    }
}

class FixedLookup extends RankedLookup {
    public String fetch(int source, int target) {
        return "path";
    }
}

class CallableFamilyIdentity {
    static int probe() {
        FixedLookup lookup = new FixedLookup();
        return lookup.fetchWeight(1, 2) + lookup.forward(1, 2).length();
    }
}
