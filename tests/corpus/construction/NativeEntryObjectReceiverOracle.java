class NativeEntryObjectReceiverOracle {
    public static void main(String[] args) {
        int alias = NativeEntryObjectReceiver.run();
        int nullable = NativeEntryObjectReceiver.nullReceiver();
        int mutation = NativeEntryObjectReceiver.nullMutation();
        int key = NativeEntryObjectReceiver.keyReceiver();
        int nullKey = NativeEntryObjectReceiver.nullKey();
        if (alias != 10 || nullable != 7 || mutation != 9 || key != 14 || nullKey != 11) {
            throw new AssertionError(alias + ":" + nullable + ":" + mutation + ":" + key + ":" + nullKey);
        }
        System.out.println(alias + ":" + nullable + ":" + mutation + ":" + key + ":" + nullKey);
    }
}
