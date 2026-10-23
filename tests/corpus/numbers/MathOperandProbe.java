package mathoperand;

/** Min/max operand retention reached by Angle.Normalizer's correction. */
public class MathOperandProbe {
    static double invalid(double value) { return value - value; }
    static float invalid32(float value) { return value - value; }
    public static int run() {
        double invalid = invalid(Double.POSITIVE_INFINITY);
        long bits = Double.doubleToRawLongBits(invalid);
        if (Double.doubleToRawLongBits(Math.max(1,invalid)) != bits
                || Double.doubleToRawLongBits(Math.max(invalid,1)) != bits) return 1;
        if (Double.doubleToRawLongBits(Math.min(1,invalid)) != bits
                || Double.doubleToRawLongBits(Math.min(invalid,1)) != bits) return 2;
        double payload = Double.longBitsToDouble(0xfff8000000000011L);
        long payloadBits = Double.doubleToRawLongBits(payload);
        if (Double.doubleToRawLongBits(Math.max(1,payload)) != payloadBits
                || Double.doubleToRawLongBits(Math.min(payload,1)) != payloadBits) return 3;
        if (Double.doubleToRawLongBits(Math.max(payload,invalid)) != payloadBits
                || Double.doubleToRawLongBits(Math.min(payload,invalid)) != payloadBits) return 4;
        if (Double.doubleToRawLongBits(Math.max(-0.0,0.0)) != 0
                || Double.doubleToRawLongBits(Math.max(0.0,-0.0)) != 0) return 5;
        if (Double.doubleToRawLongBits(Math.min(-0.0,0.0)) != Long.MIN_VALUE
                || Double.doubleToRawLongBits(Math.min(0.0,-0.0)) != Long.MIN_VALUE) return 6;
        float invalid32 = invalid32(Float.POSITIVE_INFINITY);
        int bits32 = Float.floatToRawIntBits(invalid32);
        if (Float.floatToRawIntBits(Math.max(1f,invalid32)) != bits32
                || Float.floatToRawIntBits(Math.max(invalid32,1f)) != bits32) return 7;
        if (Float.floatToRawIntBits(Math.min(1f,invalid32)) != bits32
                || Float.floatToRawIntBits(Math.min(invalid32,1f)) != bits32) return 8;
        if (Float.floatToRawIntBits(Math.max(-0.0f,0.0f)) != 0
                || Float.floatToRawIntBits(Math.min(0.0f,-0.0f)) != Integer.MIN_VALUE) return 9;
        if (Math.max(3.0,-2.0) != 3 || Math.min(3.0,-2.0) != -2
                || Math.max(3f,-2f) != 3 || Math.min(3f,-2f) != -2) return 10;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
