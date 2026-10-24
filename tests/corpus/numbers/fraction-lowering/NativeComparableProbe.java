package nativecomparable;

public final class NativeComparableProbe {
    static final class Value implements Comparable<Value> {
        int number;
        Value(int number) { this.number=number; }
        @Override public int compareTo(Value other) { return Integer.compare(number,other.number); }
    }
    static int consume(Comparable<Value> value,Value other) { return value.compareTo(other); }
    static <T extends Comparable<T>> int maximum(T left,T right) { return left.compareTo(right); }
    public static int run() {
        Value value=new Value(7);
        Value equal=new Value(7);
        Value lower=new Value(3);
        Comparable<Value> view=value;
        if (view.compareTo(equal)!=0 || view.compareTo(lower)!=1 || lower.compareTo(value)!=-1) return 1;
        Comparable<Value> alias=view;
        if (consume(alias,lower)!=1) return 2;
        value.number=-9;
        if (view.compareTo(lower)!=-1 || alias.compareTo(equal)!=-1) return 3;
        if ((Object)view!=(Object)value || (Object)alias!=(Object)value) return 4;
        if (maximum("a","b")!=-1 || maximum("b","b")!=0) return 5;
        return 0;
    }
    public static void main(String[] args) {
        int result=run();
        if (result!=0) throw new AssertionError("Comparable: "+result);
        System.out.println(result);
    }
}
