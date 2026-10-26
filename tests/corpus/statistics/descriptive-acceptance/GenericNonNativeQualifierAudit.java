package genericnonnativequalifiers;
public class GenericNonNativeQualifierAudit {
    interface Range<A,R> { R apply(A values, int from, int to); }
    static int[] keep(int[] values, int from, int to) {
        if(from<0 || to>values.length || from>to) throw new IllegalArgumentException("range");
        if(from<to) values[from]+=to-from;
        return values;
    }
    static final class Payload { int value; Payload(int value) { this.value=value; } }
    static Payload samePayload(Payload value,int from,int to) { return value; }
    static String same(String value,int from,int to) { return value; }
    static final class Builder {
        private static final int[] SEED={1,2};
        private static final int[] EMPTY={};
        private Range<int[],int[]> factory;
        void select() { factory=GenericNonNativeQualifierAudit::keep; }
        int[] build() { return create(factory,SEED,0,2); }
        int[] empty() { return create(factory,EMPTY,0,0); }
        int[] build(int[] values,int from,int to) { return create(factory,values,from,to); }
        private static <S,T> T create(Range<S,T> factory,S values,int from,int to) {
            return factory.apply(values,from,to);
        }
    }
    public static int run() {
        Builder builder=new Builder();
        builder.select();
        int[] first=builder.build();
        if(first[0]!=3 || first[1]!=2) return 1;
        if(builder.build()!=first || first[0]!=5) return 2;
        int[] empty=builder.empty();
        if(empty.length!=0 || builder.empty()!=empty) return 3;
        int[] local={7,8};
        int[] other=builder.build(local,1,2);
        if(other!=local || other==first || local[1]!=9 || first[1]!=2) return 4;
        other[0]=13;
        if(builder.build(local,0,0)!=local || local[0]!=13) return 5;
        try { builder.build(local,2,1); return 6; }
        catch(IllegalArgumentException expected) { if(local[0]!=13 || local[1]!=9) return 7; }
        try { builder.build(null,0,0); return 8; }
        catch(NullPointerException expected) { if(first[0]!=5 || first[1]!=2) return 9; }
        Range<String,String> words=GenericNonNativeQualifierAudit::same;
        String text="check";
        if(Builder.create(words,text,0,0)!=text) return 10;
        Payload payload=new Payload(5);
        Range<Payload,Payload> objects=GenericNonNativeQualifierAudit::samePayload;
        Payload alias=Builder.create(objects,payload,0,0);
        if(alias!=payload || alias.value!=5) return 11;
        alias.value=9; if(payload.value!=9) return 12;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
