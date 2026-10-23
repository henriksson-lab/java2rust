package consumer;

import producer.Datum;

public class CrossUnitNpeProbe {
    static int read(Datum value) { return value.getValue(); }
    public static int run() {
        try { read(null); return 1; }
        catch (NullPointerException e) {
            if (!e.getMessage().equals("Cannot invoke \"producer.Datum.getValue()\" because \"<parameter1>\" is null")) return 2;
        }
        if (read(new Datum()) != 7) return 3;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
