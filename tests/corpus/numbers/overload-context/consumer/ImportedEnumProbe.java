package consumer;
import producer.OverloadedEnum;

public class ImportedEnumProbe {
    public static int run() {
        OverloadedEnum value = OverloadedEnum.FIRST;
        if (value.call(2,3) != 5) return 1;
        if (value.call(2,3,4) != 9) return 2;
        if (value.call(new double[]{7,8}) != 7) return 3;
        try { value.call((double[])null); return 4; }
        catch (NullPointerException expected) { }
        if (value.call(2,3) != 5) return 5;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
