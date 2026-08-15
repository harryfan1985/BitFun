import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Alert, Button, ConfigPageLoading, Switch, Badge } from '@/component-library';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageRow,
  ConfigPageSection,
} from './common';
import { buddyAPI, type BuddyConfig as BuddyConfigData, type BuddyStatusResponse } from '@/infrastructure/api';
import { isTauriRuntime } from '@/infrastructure/runtime';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { lazy, Suspense } from 'react';

const log = createLogger('BuddyConfig');

const BuddyPairingWizard = lazy(() =>
  import('@/app/components/BuddyPairingWizard').then((m) => ({ default: m.default }))
);

const DEFAULT_CONFIG: BuddyConfigData = { enabled: false };

const BuddyConfig: React.FC = () => {
  const { t } = useTranslation('settings/buddy');
  const { error: notifyError, success: notifySuccess } = useNotification();
  const desktopRuntime = isTauriRuntime();

  const [loading, setLoading] = useState(desktopRuntime);
  const [config, setConfig] = useState<BuddyConfigData>(DEFAULT_CONFIG);
  const [status, setStatus] = useState<BuddyStatusResponse | null>(null);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [wizardOpen, setWizardOpen] = useState(false);

  const loadData = useCallback(async () => {
    if (!desktopRuntime) return;
    setLoading(true);
    try {
      const [cfg, sts] = await Promise.all([
        buddyAPI.getConfig(),
        buddyAPI.getStatus(),
      ]);
      setConfig(cfg);
      setStatus(sts);
    } catch (error) {
      log.error('Failed to load buddy config', error);
    } finally {
      setLoading(false);
    }
  }, [desktopRuntime]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  // Poll status every 10s when enabled
  useEffect(() => {
    if (!desktopRuntime || !config.enabled) return;
    const interval = setInterval(async () => {
      try {
        const sts = await buddyAPI.getStatus();
        setStatus(sts);
      } catch {
        // ignore polling errors
      }
    }, 10000);
    return () => clearInterval(interval);
  }, [desktopRuntime, config.enabled]);

  const handleSave = useCallback(async (enabled: boolean) => {
    const next = { enabled };
    setConfig(next);
    setSaving(true);
    try {
      await buddyAPI.setConfig(next);
      notifySuccess(t('messages.saved'));
    } catch (error) {
      notifyError(error instanceof Error ? error.message : t('messages.saveFailed'));
      setConfig(config);
    } finally {
      setSaving(false);
    }
  }, [config, notifyError, notifySuccess, t]);

  const handleTestConnection = useCallback(async () => {
    setTesting(true);
    try {
      const ok = await buddyAPI.testConnection();
      if (ok) {
        notifySuccess(t('messages.testSuccess'));
      } else {
        notifyError(t('messages.testFailed'));
      }
    } catch (error) {
      notifyError(error instanceof Error ? error.message : t('messages.testFailed'));
    } finally {
      setTesting(false);
    }
  }, [notifyError, notifySuccess, t]);

  if (!desktopRuntime) {
    return (
      <ConfigPageLayout>
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <ConfigPageSection title={t('desktopOnly.title')} description={t('desktopOnly.description')}>
            {null}
          </ConfigPageSection>
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  if (loading) {
    return (
      <ConfigPageLayout>
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <ConfigPageLoading text={t('loading')} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  const stateLabel = status
    ? t(`state.${status.state}`, { defaultValue: status.state })
    : t('state.not_configured');

  return (
    <ConfigPageLayout>
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />

      <ConfigPageContent>
        <ConfigPageSection
          title={t('general.title')}
          description={t('general.description')}
          titleSuffix={
            status?.enabled ? (
              <Badge variant={status.bridgeOnline ? 'success' : 'warning'}>
                {stateLabel}
              </Badge>
            ) : undefined
          }
        >
          <ConfigPageRow
            label={t('general.enable.label')}
            description={t('general.enable.description')}
            align="center"
            balanced
          >
            <Switch
              checked={config.enabled}
              disabled={saving}
              onChange={(e) => void handleSave(e.target.checked)}
            />
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection title={t('status.title')} description={t('status.description')}>
          <ConfigPageRow
            label={t('status.bridge.label')}
            description={t('status.bridge.description')}
            align="center"
            balanced
          >
            <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
              <Badge variant={status?.bridgeOnline ? 'success' : 'error'}>
                {status?.bridgeOnline ? t('status.online') : t('status.offline')}
              </Badge>
              {status?.deviceName && (
                <Badge variant={status.deviceConnected ? 'success' : 'warning'}>
                  {status.deviceName}
                </Badge>
              )}
              {status && status.pendingPrompts > 0 && (
                <Badge variant="info">
                  {t('status.pending', { count: status.pendingPrompts })}
                </Badge>
              )}
            </div>
          </ConfigPageRow>

          <ConfigPageRow
            label={t('status.test.label')}
            description={t('status.test.description')}
            align="center"
            balanced
          >
            <Button
              variant="secondary"
              size="small"
              disabled={testing || !config.enabled}
              onClick={() => void handleTestConnection()}
            >
              {testing ? t('status.test.testing') : t('status.test.button')}
            </Button>
          </ConfigPageRow>

          <ConfigPageRow
            label={t('wizard.open')}
            description={t('wizard.title')}
            align="center"
            balanced
          >
            <Button
              variant="primary"
              size="small"
              onClick={() => setWizardOpen(true)}
            >
              {t('wizard.open')}
            </Button>
          </ConfigPageRow>
        </ConfigPageSection>

        {!config.enabled && (
          <Alert
            type="info"
            message={t('alert.disabled')}
          />
        )}
      </ConfigPageContent>

      {wizardOpen && (
        <Suspense fallback={null}>
          <BuddyPairingWizard
            isOpen={wizardOpen}
            onClose={() => setWizardOpen(false)}
            onComplete={() => {
              setWizardOpen(false);
              void loadData();
            }}
          />
        </Suspense>
      )}
    </ConfigPageLayout>
  );
};

export default BuddyConfig;
