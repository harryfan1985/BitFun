import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export interface BuddyConfig {
  enabled: boolean;
}

export interface BuddyStatusResponse {
  enabled: boolean;
  state: string;
  bridgeOnline: boolean;
  deviceName: string | null;
  deviceConnected: boolean;
  pendingPrompts: number;
}

export interface BuddyPrerequisites {
  /** Whether the current host OS supports Buddy (currently macOS only). */
  supported: boolean;
  /** Host OS identifier: "macos", "windows", or "linux". */
  os: string;
}

export class BuddyAPI {
  async getConfig(): Promise<BuddyConfig> {
    try {
      return await api.invoke<BuddyConfig>('buddy_get_config');
    } catch (error) {
      throw createTauriCommandError('buddy_get_config', error);
    }
  }

  async setConfig(config: BuddyConfig): Promise<void> {
    try {
      await api.invoke<void>('buddy_set_config', { request: config });
    } catch (error) {
      throw createTauriCommandError('buddy_set_config', error, config);
    }
  }

  async getStatus(): Promise<BuddyStatusResponse> {
    try {
      return await api.invoke<BuddyStatusResponse>('buddy_get_status');
    } catch (error) {
      throw createTauriCommandError('buddy_get_status', error);
    }
  }

  async testConnection(): Promise<boolean> {
    try {
      return await api.invoke<boolean>('buddy_test_connection');
    } catch (error) {
      throw createTauriCommandError('buddy_test_connection', error);
    }
  }

  async checkPrerequisites(): Promise<BuddyPrerequisites> {
    try {
      return await api.invoke<BuddyPrerequisites>('buddy_check_prerequisites');
    } catch (error) {
      throw createTauriCommandError('buddy_check_prerequisites', error);
    }
  }
}

export const buddyAPI = new BuddyAPI();
