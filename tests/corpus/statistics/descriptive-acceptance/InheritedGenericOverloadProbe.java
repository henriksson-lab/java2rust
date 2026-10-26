package inheritedgenericoverload;
interface Multiply<T> { T multiply(T value); }
interface SmallOperators<T> extends Multiply<T> { T multiply(int count); }
interface Redeclared<T> extends Multiply<T> { T multiply(T value); T multiply(int count); }
class Value implements SmallOperators<Value>, Redeclared<Value> {
    final int n;
    Value(int n){this.n=n;}
    public Value multiply(Value other){return new Value(n*other.n);}
    public Value multiply(int count){return new Value(n*count);}
    Value self(){return this;}
}
public class InheritedGenericOverloadProbe {
    public static int run(){
        Value value=new Value(3);
        SmallOperators<Value> view=value;
        if(view.multiply(new Value(4)).n!=12 || view.multiply(5).n!=15) return 1;
        Multiply<Value> root=view;
        if(root.multiply(new Value(7)).n!=21) return 2;
        if(value.self()!=value || value.n!=3) return 3;
        Redeclared<Value> explicit=value;
        Multiply<Value> fromExplicit=explicit;
        if(fromExplicit.multiply(new Value(11)).n!=33 || explicit.multiply(2).n!=6) return 4;
        return 0;
    }
    public static void main(String[] args){System.out.println(run());}
}
