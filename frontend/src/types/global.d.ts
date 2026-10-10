import type { IconNode } from 'lucide';
declare global {
  interface Window {
    lucide: {
      createIcons(options?: {
        attrs?: Record<string, string | number>;
        icons?: Record<string, IconNode>;
      }): void;
    };
  }
}
export {};
