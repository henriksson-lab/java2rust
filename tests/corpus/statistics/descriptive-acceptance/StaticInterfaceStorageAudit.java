package staticinterfacestorage;
public class StaticInterfaceStorageAudit {
    interface Processor { int[] apply(int[] values, int[] bounds); }
    interface Child extends Processor { }
    static final class Keep implements Child {
        final boolean copy;
        Keep(boolean copy) { this.copy=copy; }
        public int[] apply(int[] values, int[] bounds) {
            bounds[0]=values.length;
            return copy ? values.clone() : values;
        }
    }
    static final class Trim implements Processor {
        final boolean copy;
        Trim(boolean copy) { this.copy=copy; }
        public int[] apply(int[] values, int[] bounds) {
            int[] result=copy ? values.clone() : values;
            result[0]=result[0]+1;
            bounds[0]=result.length-1;
            return result;
        }
    }
    static final class Reject implements Processor {
        final int limit;
        Reject(int limit) { this.limit=limit; }
        public int[] apply(int[] values, int[] bounds) {
            if(values[0]<limit) throw new IllegalArgumentException("small");
            bounds[0]=values.length;
            return values;
        }
    }
    static Processor make(int mode, boolean copy) {
        if(mode==0) return new Keep(copy);
        if(mode==1) return new Trim(copy);
        return new Reject(0);
    }
    static final class Holder {
        static final Holder DEFAULT=new Holder(0,false);
        final Processor processor;
        Holder(int mode,boolean copy) { processor=make(mode,copy); }
        int[] apply(int[] values,int[] bounds) { return processor.apply(values,bounds); }
    }
    public static int run() {
        int[] values={5,7,9}; int[] bounds={-1};
        Holder first=Holder.DEFAULT; Holder alias=first;
        int[] retained=first.apply(values,bounds);
        if(retained!=values || bounds[0]!=3 || first!=alias) return 1;
        retained[1]=11;
        if(alias.apply(values,bounds)[1]!=11) return 2;
        Holder copied=new Holder(0,true);
        int[] clone=copied.apply(values,bounds);
        if(clone==values || clone[1]!=11 || bounds[0]!=3) return 3;
        clone[1]=99;
        if(values[1]!=11) return 4;
        Holder trim=new Holder(1,false);
        if(trim.apply(values,bounds)!=values || values[0]!=6 || bounds[0]!=2) return 5;
        Holder copyTrim=new Holder(1,true);
        int[] changed=copyTrim.apply(values,bounds);
        if(changed==values || changed[0]!=7 || values[0]!=6 || bounds[0]!=2) return 6;
        Holder reject=new Holder(2,false);
        values[0]=-1; bounds[0]=-7;
        try { reject.apply(values,bounds); return 7; }
        catch(IllegalArgumentException expected) { if(bounds[0]!=-7) return 8; }
        values[0]=8;
        if(reject.apply(values,bounds)!=values || bounds[0]!=3) return 9;
        if(first.apply(values,bounds)!=values || values[0]!=8) return 10;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
