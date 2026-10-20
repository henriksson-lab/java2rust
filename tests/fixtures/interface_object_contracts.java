import java.util.*;

interface ObjectContractValue { int value(); }

class ObjectContractIdentity implements ObjectContractValue {
    public int value() { return 1; }
    public int hashCode() { return 1; }
}

class ObjectContractStructural implements ObjectContractValue {
    int number;
    boolean fail;
    ObjectContractStructural(int number, boolean fail) { this.number = number; this.fail = fail; }
    public int value() { return number; }
    public boolean equals(Object other) {
        if (fail) throw new IllegalStateException("equals");
        return other instanceof ObjectContractValue && number == ((ObjectContractValue) other).value();
    }
    public int hashCode() { return number; }
}

class InterfaceObjectContracts {
    static int run() {
        ObjectContractValue identity = new ObjectContractIdentity();
        ObjectContractValue alias = identity;
        ObjectContractValue distinct = new ObjectContractIdentity();
        ObjectContractValue structural = new ObjectContractStructural(1, false);
        ObjectContractValue throwing = new ObjectContractStructural(1, true);
        if (identity != alias || identity == distinct) return 1;
        if (!identity.equals(alias) || identity.equals(distinct)) return 2;
        if (!structural.equals(identity) || identity.equals(structural)) return 3;
        if (structural.hashCode() != 1) return 4;
        Set<ObjectContractValue> values = new LinkedHashSet<>();
        values.add(identity);
        if (!values.contains(structural)) return 5;
        try { values.contains(throwing); return 6; }
        catch (IllegalStateException expected) { }
        if (alias != identity || values.size() != 1) return 7;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
