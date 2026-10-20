import java.util.*;

class HashContractToken {
    int code;
    HashContractToken(int code) { this.code = code; }
    public int hashCode() { return code; }
}

class HashContractHolder<T> {
    final T value;
    HashContractHolder(T value) { this.value = value; }
    public int hashCode() { return value.hashCode(); }
}

class HashContractPlain {
    int value;
}

class HashContractThrowing {
    public int hashCode() { throw new IllegalStateException("hash"); }
}

class HashContractCounting {
    int calls;
    public int hashCode() { calls++; return 47; }
}

class HashContractImmutable { }

class JavaHashContracts {
    static <T> int hash(T value) { return value.hashCode(); }
    static <T> int setHash(Set<T> values) { return values.hashCode(); }
    static int run() {
        List<String> list = new ArrayList<>();
        list.add("a");
        list.add("b");
        if (list.hashCode() != 4066) return 1;
        Set<String> plain = new HashSet<>();
        plain.add("a");
        plain.add("b");
        if (plain.hashCode() != 195) return 2;
        Set<String> retained = new HashSet<>();
        retained.add("a");
        retained.add("b");
        Set<String> alias = retained;
        if (alias != retained || retained.hashCode() != 195 || setHash(alias) != 195) return 3;
        retained.add(null);
        if (retained.hashCode() != 195) return 4;
        Map<String,String> values = new HashMap<>();
        values.put("a", "b");
        values.put("b", "a");
        if (values.hashCode() != 6) return 5;
        HashContractToken token = new HashContractToken(47);
        if (hash(token) != 47 || hash("ab") != 3105 || hash(Integer.valueOf(7)) != 7) return 6;
        Set<HashContractToken> tokens = new HashSet<>();
        tokens.add(token);
        if (tokens.hashCode() != 47) return 7;
        if (hash(Double.valueOf(-0.0)) != Integer.MIN_VALUE || hash(Boolean.TRUE) != 1231) return 8;
        HashContractHolder<HashContractToken> holder = new HashContractHolder<>(token);
        if (hash(holder) != 47) return 9;
        HashContractPlain object = new HashContractPlain();
        HashContractPlain objectAlias = object;
        int before = hash(object);
        objectAlias.value = 2;
        if (object != objectAlias || hash(object) != before || hash(objectAlias) != before) return 10;
        try { hash(new HashContractThrowing()); return 11; }
        catch (IllegalStateException expected) { }
        HashContractCounting counting = new HashContractCounting();
        if (hash(counting) != 47 || counting.calls != 1) return 12;
        if (hash(counting) != 47 || counting.calls != 2) return 13;
        try { hash((String)null); return 14; }
        catch (NullPointerException expected) { }
        HashContractImmutable immutable = new HashContractImmutable();
        int stable = hash(immutable);
        if (hash(immutable) != stable) return 15;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
