import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal, Button, Alert, Badge } from '@/component-library';
import { buddyAPI, type BuddyPrerequisites } from '@/infrastructure/api';
import { useNotification } from '@/shared/notification-system';

type WizardStep = 'prerequisites' | 'bridge_start' | 'device_scan' | 'test';

interface BuddyPairingWizardProps {
  isOpen: boolean;
  onClose: () => void;
  onComplete: () => void;
}

const BuddyPairingWizard: React.FC<BuddyPairingWizardProps> = ({ isOpen, onClose, onComplete }) => {
  const { t } = useTranslation('settings/buddy');
  const { error: notifyError, success: notifySuccess } = useNotification();

  const [step, setStep] = useState<WizardStep>('prerequisites');
  const [prereqs, setPrereqs] = useState<BuddyPrerequisites | null>(null);
  const [checkingPrereqs, setCheckingPrereqs] = useState(false);
  const [deviceName, setDeviceName] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [testing, setTesting] = useState(false);

  const reset = useCallback(() => {
    setStep('prerequisites');
    setPrereqs(null);
    setDeviceName(null);
    setScanning(false);
    setTesting(false);
  }, []);

  useEffect(() => {
    if (isOpen) reset();
  }, [isOpen, reset]);

  const checkPrerequisites = useCallback(async () => {
    setCheckingPrereqs(true);
    try {
      const result = await buddyAPI.checkPrerequisites();
      setPrereqs(result);
      if (result.supported) {
        setStep('bridge_start');
      }
    } catch (error) {
      notifyError(error instanceof Error ? error.message : t('wizard.prerequisites.failed'));
    } finally {
      setCheckingPrereqs(false);
    }
  }, [notifyError, t]);

  const startBridge = useCallback(() => {
    // Bridge start is handled by the managed mode boot wiring
    setStep('device_scan');
  }, []);

  const scanForDevice = useCallback(async () => {
    setScanning(true);
    try {
      const status = await buddyAPI.getStatus();
      if (status.deviceConnected && status.deviceName) {
        setDeviceName(status.deviceName);
        setStep('test');
      } else {
        // Poll for up to 30 seconds
        let attempts = 0;
        const interval = setInterval(async () => {
          attempts++;
          try {
            const sts = await buddyAPI.getStatus();
            if (sts.deviceConnected && sts.deviceName) {
              setDeviceName(sts.deviceName);
              setStep('test');
              clearInterval(interval);
              setScanning(false);
            } else if (attempts >= 15) {
              clearInterval(interval);
              setScanning(false);
              notifyError(t('wizard.device.notFound'));
            }
          } catch {
            // ignore polling errors
          }
        }, 2000);
      }
    } catch (error) {
      setScanning(false);
      notifyError(error instanceof Error ? error.message : t('wizard.device.scanFailed'));
    }
  }, [notifyError, t]);

  const testConnection = useCallback(async () => {
    setTesting(true);
    try {
      await buddyAPI.testConnection();
      notifySuccess(t('wizard.test.success'));
      onComplete();
    } catch (error) {
      notifyError(error instanceof Error ? error.message : t('wizard.test.failed'));
    } finally {
      setTesting(false);
    }
  }, [notifyError, notifySuccess, onComplete, t]);

  const stepOrder: WizardStep[] = ['prerequisites', 'bridge_start', 'device_scan', 'test'];
  const currentStepIndex = stepOrder.indexOf(step);

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('wizard.title')}
      size="medium"
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16, padding: '8px 0' }}>
        {/* Progress indicator */}
        <div style={{ display: 'flex', gap: 4 }}>
          {stepOrder.map((s, i) => (
            <div
              key={s}
              style={{
                flex: 1,
                height: 3,
                borderRadius: 2,
                background: i <= currentStepIndex ? 'var(--color-accent)' : 'var(--color-border)',
              }}
            />
          ))}
        </div>

        {/* Step 1: Prerequisites */}
        {step === 'prerequisites' && (
          <div>
            <h3 style={{ margin: '0 0 12px' }}>{t('wizard.prerequisites.title')}</h3>
            {prereqs && !prereqs.supported ? (
              <Alert type="warning" message={t('wizard.prerequisites.unsupported')} />
            ) : (
              <>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 8, marginBottom: 16 }}>
                  <PrereqItem
                    label={t('wizard.prerequisites.macos')}
                    ok={prereqs?.supported ?? null}
                  />
                </div>
                <Button
                  variant="primary"
                  size="small"
                  disabled={checkingPrereqs}
                  onClick={() => void checkPrerequisites()}
                >
                  {checkingPrereqs ? t('wizard.prerequisites.checking') : t('wizard.prerequisites.check')}
                </Button>
              </>
            )}
          </div>
        )}

        {/* Step 2: Bridge Start */}
        {step === 'bridge_start' && (
          <div>
            <h3 style={{ margin: '0 0 12px' }}>{t('wizard.bridge.title')}</h3>
            <p style={{ color: 'var(--color-text-secondary)', fontSize: 13, margin: '0 0 16px' }}>
              {t('wizard.bridge.description')}
            </p>
            <Alert type="info" message={t('wizard.bridge.managed')} />
            <div style={{ marginTop: 16 }}>
              <Button variant="primary" size="small" onClick={startBridge}>
                {t('wizard.bridge.continue')}
              </Button>
            </div>
          </div>
        )}

        {/* Step 3: Device Scan */}
        {step === 'device_scan' && (
          <div>
            <h3 style={{ margin: '0 0 12px' }}>{t('wizard.device.title')}</h3>
            <p style={{ color: 'var(--color-text-secondary)', fontSize: 13, margin: '0 0 16px' }}>
              {t('wizard.device.description')}
            </p>
            {scanning ? (
              <div style={{ textAlign: 'center', padding: '24px 0' }}>
                <div style={{ fontSize: 14, color: 'var(--color-text-secondary)' }}>
                  {t('wizard.device.scanning')}
                </div>
              </div>
            ) : (
              <Button variant="primary" size="small" onClick={() => void scanForDevice()}>
                {t('wizard.device.scan')}
              </Button>
            )}
          </div>
        )}

        {/* Step 4: Test */}
        {step === 'test' && (
          <div>
            <h3 style={{ margin: '0 0 12px' }}>{t('wizard.test.title')}</h3>
            {deviceName && (
              <div style={{ marginBottom: 12 }}>
                <Badge variant="success">{deviceName}</Badge>
              </div>
            )}
            <p style={{ color: 'var(--color-text-secondary)', fontSize: 13, margin: '0 0 16px' }}>
              {t('wizard.test.description')}
            </p>
            <Button
              variant="primary"
              size="small"
              disabled={testing}
              onClick={() => void testConnection()}
            >
              {testing ? t('wizard.test.testing') : t('wizard.test.button')}
            </Button>
          </div>
        )}
      </div>
    </Modal>
  );
};

const PrereqItem: React.FC<{ label: string; ok: boolean | null }> = ({ label, ok }) => (
  <div style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13 }}>
    <span style={{ color: ok === null ? 'var(--color-text-tertiary)' : ok ? 'var(--color-success)' : 'var(--color-error)' }}>
      {ok === null ? '\u25CB' : ok ? '\u2713' : '\u2717'}
    </span>
    <span>{label}</span>
  </div>
);

export default BuddyPairingWizard;
