public class L1 { public int run(int n){ int t=0; for(int i=0;i<n;i++){ for(int j=0;j<n;j++){ if(j==1) continue; t+=1; } } return t; } }
