public class StringBuilderSetLengthCatch {
    public static String run() {
        StringBuilder builder = new StringBuilder("abc");
        builder.setLength(5);
        builder.setLength(2);
        String builderValue = builder.toString();
        String builderStatus;
        try {
            builder.setLength(-1);
            builderStatus = "miss";
        } catch (StringIndexOutOfBoundsException e) {
            builderStatus = "builder";
        }

        StringBuffer buffer = new StringBuffer("xy");
        buffer.setLength(4);
        buffer.setLength(1);
        String bufferValue = buffer.toString();
        String bufferStatus;
        try {
            buffer.setLength(-2);
            bufferStatus = "miss";
        } catch (StringIndexOutOfBoundsException e) {
            bufferStatus = "buffer";
        }

        return builderValue.length() + ":" + builderStatus + ":" + bufferValue.length() + ":" + bufferStatus;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
