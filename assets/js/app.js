const apiBase = '/fiberspider/api';
const LANG_KEY = 'photaxonLang';
let currentLang = localStorage.getItem(LANG_KEY) || 'it';

const translations = {
  it: {
    brandTitle: 'PHOTAXON',
    brandSubtitle: 'FiberSpider Laboratory',
    heroEyebrow: 'Laboratory Simulation',
    heroTitle: 'Rete FTTH distribuita con nodi intelligenti e AI.',
    heroLead: 'FiberSpider unisce telemetria reale, LLM intelligence e rete distribuita sicura.',
    heroCta1: 'Vedi il laboratorio',
    heroCta2: 'Attivazione tecnico',
    heroCta3: 'Architettura',
    heroNote: 'LABORATORY SIMULATION — Nessun provisioning FTTH reale eseguito.',
    heroCardTitle: 'Live FiberSpider Lab',
    heroCardText: 'Dati reali in tempo reale, topologia dinamica e validazione deterministica.',
    statLive: 'Live lab',
    statLiveDesc: 'API-driven network',
    statTopology: 'Topology',
    statTopologyDesc: 'Interpretazione intelligente',
    statValidation: 'Validation',
    statValidationDesc: 'Backend control',
    liveLabTitle: 'Live FiberSpider Lab',
    liveLabSubtitle: 'Stato del laboratorio e infrastruttura in tempo reale.',
    apiStatusLabel: 'API status',
    simulationLabel: 'Simulation',
    controlPlaneLabel: 'Control plane',
    generalStatus: 'General status',
    apiOnline: 'API Online',
    projectLabel: 'Project',
    statusLabel: 'Status',
    technicianLabel: 'Technician',
    ontDetectedLabel: 'ONT detected',
    missionDetails: 'Mission details',
    ontSerialLabel: 'ONT serial',
    llmProposalLabel: 'LLM proposal',
    backendValidationLabel: 'Backend validation',
    provisioningLabel: 'Provisioning',
    finalStateLabel: 'Final state',
    openstackTitle: 'OpenStack Infrastructure',
    openstackSubtitle: 'Control plane and virtual network topology.',
    fsMgmtLabel: 'FS-MGMT',
    fsLineALabel: 'FS-LINE-A',
    fsLineBLabel: 'FS-LINE-B',
    fsLineCLabel: 'FS-LINE-C',
    topologyTitle: 'Fiber topology',
    topologySubtitle: 'Percorso simulato dal laboratorio.',
    topologyNote: 'Il percorso è una rappresentazione animata della simulazione di rete.',
    llmTitle: 'FIBERSPIDER INTELLIGENCE',
    llmSubtitle: 'Il processo LLM e la validazione deterministica.',
    llmObserve: 'Observe',
    llmAnalyze: 'Analyze',
    llmPropose: 'Topology Proposal',
    llmValidate: 'Deterministic Validation',
    llmProvision: 'Provisioning Plan',
    activationTitle: 'Attivazione tecnico',
    activationSubtitle: 'Workflow di attivazione reale con OTP Telegram.',
    telegramIdLabel: 'Telegram ID',
    workRequestLabel: 'WR / Work Request',
    ontSerialLabel: 'ONT GPON / Serial',
    roeLabel: 'ROE',
    cabinetLabel: 'Cabinet',
    splitterLabel: 'Splitter',
    notesLabel: 'Field path / technician notes',
    activationButtonStart: '📲 Invia codice OTP',
    activationButtonVerify: '🔐 Conferma OTP',
    otpLabel: 'Codice OTP',
    otpPlaceholder: 'Inserisci codice OTP',
    activationNote: 'Provisioning finale rimane LABORATORY SIMULATION. Usa il backend per autenticazione e OTP.',
    activationReady: 'Compila i dati e richiedi il codice OTP.',
    activationSuccessDetailed: '✅ OTP verificato\n✅ Simulazione FiberSpider completata\n✅ Risultato registrato nel database\nJOB_CLOSED\nProvisioning FTTH fisico: NON ESEGUITO',
    activationFlowTitle: 'Flusso di attivazione',
    activationStep1: 'Identità Telegram',
    activationStep2: 'Verifica tecnico',
    activationStep3: 'Verifica WR',
    activationStep4: 'OTP Telegram',
    activationStep5: 'Identificazione ONT / GPON',
    activationStep6: 'Analisi percorso fisico',
    activationStep7: 'Analisi LLM',
    activationStep8: 'Validazione backend',
    activationStep9: 'Provisioning simulato',
    activationStep10: 'Attività chiusa',
    activationStatusAuthorized: '✅ Tecnico autorizzato',
    activationStatusStored: '✅ Richiesta registrata nel database',
    activationStatusOtpSent: '✅ Codice OTP inviato via Telegram',
    activationRefRegistered: 'RIFERIMENTO REGISTRATO',
    technicianNotAuthorized: 'Tecnico non autorizzato',
    telegramChatNotRegistered: 'Apri prima la chat privata con il bot Photaxon su Telegram.',
    otpInvalid: 'Codice OTP non valido.',
    otpExpired: 'Codice scaduto. Richiedi un nuovo OTP.',
    otpLocked: 'Troppi tentativi. Richiedi una nuova attivazione.',
    activationNotFound: 'Richiesta di attivazione non trovata.',
    activationCheckInput: 'Controlla i dati inseriti.',
    activationSuccess: 'SIMULATION_COMPLETED — JOB_CLOSED. Provisioning finale è simulato.',
    architectureTitle: 'Distributed architecture',
    architectureSubtitle: 'Public, control and data layers.',
    alexRole: 'Public Web Layer',
    meshLabel: 'Encrypted Mesh',
    ragnoRole: 'FiberSpider Intelligence / API',
    aaaRole: 'Distributed Data Layer',
    fundingTitle: 'NEXT RESEARCH PHASE',
    fundingSubtitle: 'Smart FTTH cabinet prototype.',
    research1: 'Optical Sensors',
    research2: 'Telemetry',
    research3: 'Smart Switching',
    research4: 'Local AI Compute',
    research5: 'Resilient Power',
    research6: 'Autonomous Diagnostics',
    research7: 'Distributed Control',
    fundingNote: 'Software proof of concept completed. Hardware smart cabinet is research/prototype phase.',
    openstackTitle: 'OpenStack Infrastructure',
    openstackSubtitle: 'Control plane and virtual network topology.',
  },
  en: {
    brandTitle: 'PHOTAXON',
    brandSubtitle: 'FiberSpider Laboratory',
    heroEyebrow: 'Laboratory Simulation',
    heroTitle: 'Distributed FTTH network with intelligent nodes and AI.',
    heroLead: 'FiberSpider combines real telemetry, LLM intelligence and a secure distributed mesh.',
    heroCta1: 'See the lab',
    heroCta2: 'Technician activation',
    heroCta3: 'Architecture',
    heroNote: 'LABORATORY SIMULATION — No physical FTTH provisioning executed.',
    heroCardTitle: 'Live FiberSpider Lab',
    heroCardText: 'Real-time data, dynamic topology and deterministic validation.',
    statLive: 'Live lab',
    statLiveDesc: 'API-driven network',
    statTopology: 'Topology',
    statTopologyDesc: 'Intelligent routing',
    statValidation: 'Validation',
    statValidationDesc: 'Backend control',
    liveLabTitle: 'Live FiberSpider Lab',
    liveLabSubtitle: 'Lab status and infrastructure in real time.',
    apiStatusLabel: 'API status',
    simulationLabel: 'Simulation',
    controlPlaneLabel: 'Control plane',
    generalStatus: 'General status',
    apiOnline: 'API Online',
    projectLabel: 'Project',
    statusLabel: 'Status',
    technicianLabel: 'Technician',
    ontDetectedLabel: 'ONT detected',
    missionDetails: 'Mission details',
    ontSerialLabel: 'ONT serial',
    llmProposalLabel: 'LLM proposal',
    backendValidationLabel: 'Backend validation',
    provisioningLabel: 'Provisioning',
    finalStateLabel: 'Final state',
    openstackTitle: 'OpenStack Infrastructure',
    openstackSubtitle: 'Control plane and virtual network topology.',
    fsMgmtLabel: 'FS-MGMT',
    fsLineALabel: 'FS-LINE-A',
    fsLineBLabel: 'FS-LINE-B',
    fsLineCLabel: 'FS-LINE-C',
    topologyTitle: 'Fiber topology',
    topologySubtitle: 'Simulated lab path.',
    topologyNote: 'The route is an animated representation of network simulation.',
    llmTitle: 'FIBERSPIDER INTELLIGENCE',
    llmSubtitle: 'LLM process and deterministic validation.',
    llmObserve: 'Observe',
    llmAnalyze: 'Analyze',
    llmPropose: 'Topology Proposal',
    llmValidate: 'Deterministic Validation',
    llmProvision: 'Provisioning Plan',
    activationTitle: 'Technician activation',
    activationSubtitle: 'Real activation workflow with Telegram OTP.',
    telegramIdLabel: 'Telegram ID',
    workRequestLabel: 'WR / Work Request',
    ontSerialLabel: 'ONT GPON / Serial',
    roeLabel: 'ROE',
    cabinetLabel: 'Cabinet',
    splitterLabel: 'Splitter',
    notesLabel: 'Field path / technician notes',
    activationButtonStart: '📲 Send OTP code',
    activationButtonVerify: '🔐 Confirm OTP',
    otpLabel: 'OTP Code',
    otpPlaceholder: 'Enter OTP code',
    activationNote: 'Final provisioning remains LABORATORY SIMULATION. Use the backend for authentication and OTP.',
    activationReady: 'Enter the activation data and request an OTP code.',
    activationSuccessDetailed: '✅ OTP verified\n✅ FiberSpider simulation completed\n✅ Result stored in the database\nJOB_CLOSED\nPhysical FTTH provisioning: NOT EXECUTED',
    activationFlowTitle: 'Activation workflow',
    activationStep1: 'Telegram Identity',
    activationStep2: 'Technician verification',
    activationStep3: 'WR verification',
    activationStep4: 'OTP Telegram',
    activationStep5: 'ONT / GPON identification',
    activationStep6: 'Physical path analysis',
    activationStep7: 'LLM analysis',
    activationStep8: 'Backend validation',
    activationStep9: 'Simulated provisioning',
    activationStep10: 'Job closed',
    activationStatusAuthorized: '✅ Technician authorized',
    activationStatusStored: '✅ Request stored in database',
    activationStatusOtpSent: '✅ OTP code sent via Telegram',
    activationRefRegistered: 'REGISTERED REFERENCE',
    technicianNotAuthorized: 'Technician not authorized',
    telegramChatNotRegistered: 'Open the private chat with the Photaxon Telegram bot first.',
    otpInvalid: 'Invalid OTP code.',
    otpExpired: 'OTP code expired. Request a new one.',
    otpLocked: 'Too many attempts. Request a new activation.',
    activationNotFound: 'Activation request not found.',
    activationCheckInput: 'Check the entered data.',
    activationSuccess: 'SIMULATION_COMPLETED — JOB_CLOSED. Final provisioning is simulated.',
    architectureTitle: 'Distributed architecture',
    architectureSubtitle: 'Public, control and data layers.',
    alexRole: 'Public Web Layer',
    meshLabel: 'Encrypted Mesh',
    ragnoRole: 'FiberSpider Intelligence / API',
    aaaRole: 'Distributed Data Layer',
    fundingTitle: 'NEXT RESEARCH PHASE',
    fundingSubtitle: 'Smart FTTH cabinet prototype.',
    research1: 'Optical Sensors',
    research2: 'Telemetry',
    research3: 'Smart Switching',
    research4: 'Local AI Compute',
    research5: 'Resilient Power',
    research6: 'Autonomous Diagnostics',
    research7: 'Distributed Control',
    fundingNote: 'Software proof of concept completed. Hardware smart cabinet is research/prototype phase.',
  }
};
const elements = {
  apiStatusText: document.getElementById('apiStatusText'),
  apiStatusDesc: document.getElementById('apiStatusDesc'),
  healthStatus: document.getElementById('healthStatus'),
  projectName: document.getElementById('projectName'),
  projectState: document.getElementById('projectState'),
  techAuth: document.getElementById('techAuth'),
  ontDetected: document.getElementById('ontDetected'),
  ontSerial: document.getElementById('ontSerial'),
  llmProposal: document.getElementById('llmProposal'),
  backendValidation: document.getElementById('backendValidation'),
  provisioningMode: document.getElementById('provisioningMode'),
  finalState: document.getElementById('finalState'),
  simulationStatus: document.getElementById('simulationStatus'),
  simulationDesc: document.getElementById('simulationDesc'),
  openstackStatus: document.getElementById('openstackStatus'),
  openstackStatusDesc: document.getElementById('openstackStatusDesc'),
  fsMgmtStatus: document.getElementById('fsMgmtStatus'),
  fsLineAStatus: document.getElementById('fsLineAStatus'),
  fsLineBStatus: document.getElementById('fsLineBStatus'),
  fsLineCStatus: document.getElementById('fsLineCStatus'),
  cabAStatus: document.getElementById('cabAStatus'),
  cabBStatus: document.getElementById('cabBStatus'),
  cabCStatus: document.getElementById('cabCStatus'),
  activationForm: document.getElementById('activationForm'),
  telegramId: document.getElementById('telegramId'),
  workRequest: document.getElementById('workRequest'),
  ontSerialInput: document.getElementById('ontSerialInput'),
  roeInput: document.getElementById('roeInput'),
  sendOtpButton: document.getElementById('sendOtpButton'),
  verifyOtpButton: document.getElementById('verifyOtpButton'),
  otpFieldWrapper: document.getElementById('otpFieldWrapper'),
  otpInput: document.getElementById('otpInput'),
  cabinetInput: document.getElementById('cabinetInput'),
  splitterInput: document.getElementById('splitterInput'),
  notesInput: document.getElementById('notesInput'),
  activationResult: document.getElementById('activationResult'),
  activationSteps: document.getElementById('activationSteps'),
  topologyCanvas: document.getElementById('topologyCanvas'),
  backgroundCanvas: document.getElementById('bgCanvas'),
  llmSteps: Array.from(document.querySelectorAll('.llm-step')),
  llmStatusText: document.getElementById('llmStatusText'),
};

const activationState = {
  activationId: sessionStorage.getItem('photaxonActivationId') || null,
  step: null,
  messageKey: null,
  errorKey: null,
  cabinet: 'CAB-A',
  path: [],
  finalState: null,
};

const nodes = {
  CENTRAL: 'Data center',
  CABINET: 'Cabinet',
  'FTTH-LINE': 'FTTH line',
  SPLITTER: 'Splitter',
  ROE: 'ROE',
  ONT: 'ONT',
};

function mapTopologyNode(label) {
  if (!label) return 'NODE';
  const key = String(label).toUpperCase();
  if (key.includes('CAB')) return 'CABINET';
  if (key.includes('ROE')) return 'ROE';
  if (key.includes('ONT')) return 'ONT';
  if (key.includes('LINE')) return 'LINE';
  if (key.includes('SPL')) return 'SPLITTER';
  return 'NODE';
}

const rAF = window.requestAnimationFrame;
let bgAnimationId = null;
let topologyAnimationId = null;
let topologyState = { path: [] };

function getTranslation(key) {
  return (translations[currentLang] && translations[currentLang][key]) || translations.it[key] || key;
}

function translatePage() {
  document.querySelectorAll('[data-i18n]').forEach((el) => {
    const key = el.dataset.i18n;
    const value = getTranslation(key);
    if (el.dataset.i18nHtml === 'true') {
      el.innerHTML = value;
    } else {
      el.textContent = value;
    }
  });
}

function updateLanguageSwitcher() {
  document.querySelectorAll('[data-lang]').forEach((button) => {
    button.classList.toggle('active', button.dataset.lang === currentLang);
  });
}

function setLanguage(lang) {
  currentLang = lang;
  localStorage.setItem(LANG_KEY, lang);
  translatePage();
  updateLanguageSwitcher();
}

function initializeLanguage() {
  document.querySelectorAll('[data-lang]').forEach((button) => {
    button.addEventListener('click', () => setLanguage(button.dataset.lang));
  });
  setLanguage(currentLang);
}

function setStatus(online) {
  if (!elements.apiStatusText) return;
  elements.apiStatusText.textContent = online ? 'API ONLINE' : 'API OFFLINE';
  elements.apiStatusDesc.textContent = online ? 'FiberSpider backend is reachable.' : 'Live backend unreachable. Simulation only.';
  elements.apiStatusText.className = online ? 'status-text badge badge-success' : 'status-text badge badge-offline';
}

function normalizeLabel(value, fallback) {
  if (value === undefined || value === null) return fallback;
  return String(value).trim().toUpperCase();
}

function setOpenstackStatus(value) {
  const text = normalizeLabel(value, 'OFFLINE');
  if (!elements.openstackStatus) return;
  elements.openstackStatus.textContent = text;
  elements.openstackStatus.className = 'status-text badge ' + (text === 'ONLINE' ? 'badge-success' : text === 'OFFLINE' ? 'badge-offline' : 'badge-warning');
  elements.openstackStatusDesc.textContent = text === 'ONLINE' ? 'OpenStack control plane is reachable.' : 'OpenStack control plane unavailable.';
}

function setOpenstackValue(node, label) {
  const element = elements[node];
  if (!element) return;
  const text = normalizeLabel(label, 'MISSING');
  let css = 'badge-warning';
  if (text === 'ACTIVE' || text === 'PRESENT' || text === 'ONLINE') css = 'badge-success';
  if (text === 'OFFLINE' || text === 'MISSING' || text === 'OTHER') css = 'badge-offline';
  element.textContent = text;
  element.className = 'status-text badge ' + css;
}

function setOpenstackData(data) {
  if (!data || typeof data !== 'object') {
    return setOpenstackOffline();
  }

  const openstackInfo = data.openstack || {};
  const serverInfo = data.servers || {};
  const networkInfo = data.networks || {};

  setOpenstackStatus(openstackInfo.status || openstackInfo.state || 'OFFLINE');
  setOpenstackValue('fsMgmtStatus', networkInfo['FS-MGMT']?.present ? 'PRESENT' : 'MISSING');
  setOpenstackValue('fsLineAStatus', networkInfo['FS-LINE-A']?.present ? 'PRESENT' : 'MISSING');
  setOpenstackValue('fsLineBStatus', networkInfo['FS-LINE-B']?.present ? 'PRESENT' : 'MISSING');
  setOpenstackValue('fsLineCStatus', networkInfo['FS-LINE-C']?.present ? 'PRESENT' : 'MISSING');
  setOpenstackValue('cabAStatus', serverInfo['CAB-A']?.status || 'OTHER');
  setOpenstackValue('cabBStatus', serverInfo['CAB-B']?.status || 'OTHER');
  setOpenstackValue('cabCStatus', serverInfo['CAB-C']?.status || 'OTHER');
  setCabinetVisual('cabA', serverInfo['CAB-A']?.status || 'OTHER');
  setCabinetVisual('cabB', serverInfo['CAB-B']?.status || 'OTHER');
  setCabinetVisual('cabC', serverInfo['CAB-C']?.status || 'OTHER');
}

function setOpenstackOffline() {
  setOpenstackStatus('OFFLINE');
  setOpenstackValue('fsMgmtStatus', 'MISSING');
  setOpenstackValue('fsLineAStatus', 'MISSING');
  setOpenstackValue('fsLineBStatus', 'MISSING');
  setOpenstackValue('fsLineCStatus', 'MISSING');
  setOpenstackValue('cabAStatus', 'OTHER');
  setOpenstackValue('cabBStatus', 'OTHER');
  setOpenstackValue('cabCStatus', 'OTHER');
}

function setCabinetVisual(id, status) {
  const node = document.getElementById(id);
  if (!node) return;
  node.dataset.status = status;
  node.classList.toggle('active', status === 'ACTIVE');
  node.classList.toggle('offline', status === 'OFFLINE');
}

function updateLLMSteps(data) {
  if (!elements.llmSteps) return;
  elements.llmSteps.forEach((step) => step.classList.remove('active'));
  const active = [];
  active.push('observe');
  active.push('analyze');
  if (data?.llm?.proposal_created) active.push('propose');
  if (data?.backend?.validation === 'PASS') active.push('validate');
  if (data?.provisioning?.status) active.push('provision');
  elements.llmSteps.forEach((step) => {
    if (active.includes(step.dataset.step)) step.classList.add('active');
  });
  if (data?.final_state === 'JOB_CLOSED') {
    elements.llmStatusText.textContent = 'JOB_CLOSED — Simulation complete';
  } else {
    elements.llmStatusText.textContent = 'Processing...';
  }
}

function setStatusLabel(id, value) {
  const el = document.getElementById(id);
  if (!el) return;
  el.textContent = value;
}

function getSplitterForCabinet(cabinet) {
  switch (cabinet) {
    case 'CAB-B': return 'SPL-B-01';
    case 'CAB-C': return 'SPL-C-01';
    default: return 'SPL-A-01';
  }
}

function setActivationMessage(key) {
  activationState.messageKey = key;
  activationState.errorKey = null;
  if (elements.activationResult) {
    elements.activationResult.textContent = getTranslation(key);
  }
}

function setActivationError(key, detail) {
  activationState.errorKey = key;
  activationState.messageKey = null;
  if (!elements.activationResult) return;
  const message = getTranslation(key);
  elements.activationResult.textContent = detail ? `${message} ${detail}` : message;
}

function updateActivationTimeline(activeStep) {
  if (!elements.activationSteps) return;
  const steps = ['identity', 'technician', 'wr', 'otp', 'ont', 'physical', 'llm', 'validation', 'provision', 'closed'];
  const activeIndex = steps.indexOf(activeStep);
  Array.from(elements.activationSteps.querySelectorAll('li')).forEach((stepItem) => {
    const index = steps.indexOf(stepItem.dataset.step);
    stepItem.classList.toggle('active', index !== -1 && index <= activeIndex);
  });
}

function setOtpFlow(enabled) {
  if (elements.sendOtpButton) {
    elements.sendOtpButton.disabled = enabled;
  }
  if (elements.verifyOtpButton) {
    elements.verifyOtpButton.classList.toggle('hidden', !enabled);
    if (enabled) elements.verifyOtpButton.disabled = false;
  }
  if (elements.otpFieldWrapper) {
    elements.otpFieldWrapper.classList.toggle('hidden', !enabled);
  }
}

function resetOtpFlow() {
  activationState.activationId = null;
  sessionStorage.removeItem('photaxonActivationId');
  activationState.step = 'identity';
  updateActivationTimeline('identity');
  setOtpFlow(false);
  if (elements.otpInput) {
    elements.otpInput.value = '';
  }
  setActivationMessage('activationReady');
}

function setActivationButtonsEnabled(enabled) {
  if (elements.sendOtpButton) elements.sendOtpButton.disabled = !enabled;
  if (elements.verifyOtpButton) elements.verifyOtpButton.disabled = !enabled;
}

function collectActivationPayload() {
  return {
    telegram_id: elements.telegramId?.value.trim() || '',
    wr: elements.workRequest?.value.trim() || '',
    ont_serial: elements.ontSerialInput?.value.trim() || '',
    roe: elements.roeInput?.value.trim() || '',
    cabinet: elements.cabinetInput?.value.trim() || 'CAB-A',
    splitter: elements.splitterInput?.value.trim() || getSplitterForCabinet(elements.cabinetInput?.value.trim() || 'CAB-A'),
    notes: elements.notesInput?.value.trim() || '',
  };
}

function validateActivationPayload(payload) {
  return Boolean(payload.telegram_id && payload.wr && payload.ont_serial && payload.roe && payload.cabinet);
}

async function startActivation() {
  const result = document.getElementById('activationResult');
  if (result) {
    result.textContent = currentLang === 'en' ? 'Sending OTP request...' : 'Invio richiesta OTP...';
  }
  const payload = collectActivationPayload();
  if (!validateActivationPayload(payload)) {
    setActivationError('activationCheckInput');
    return;
  }
  setActivationButtonsEnabled(false);
  try {
    const response = await fetch(`${apiBase}/activation/start`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    const data = await response.json();
    if (response.status === 403) {
      setActivationError('technicianNotAuthorized');
      resetOtpFlow();
      return;
    }
    if (response.status === 422) {
      setActivationError('activationCheckInput');
      resetOtpFlow();
      return;
    }
    if (data?.status === 'OTP_SENT') {
      activationState.activationId = data.activation_id || data.activationId || null;
      if (activationState.activationId) {
        sessionStorage.setItem('photaxonActivationId', activationState.activationId);
      }
      activationState.step = 'otp';
      updateActivationTimeline('otp');
      setOtpFlow(true);
      setActivationMessage('activationStatusOtpSent');
      return;
    }
    setActivationError('activationCheckInput');
    setActivationButtonsEnabled(true);
  } catch (error) {
    setActivationError('activationCheckInput');
    setActivationButtonsEnabled(true);
  }
}

async function verifyActivation() {
  const activationId = activationState.activationId || sessionStorage.getItem('photaxonActivationId');
  const otp = elements.otpInput?.value.trim() || '';
  if (!activationId) {
    setActivationError('activationNotFound');
    return;
  }
  if (!otp) {
    setActivationError('activationCheckInput');
    return;
  }
  if (elements.verifyOtpButton) elements.verifyOtpButton.disabled = true;
  try {
    const response = await fetch(`${apiBase}/activation/verify`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ activation_id: activationId, otp }),
    });
    const data = await response.json();
    if (response.status === 403) {
      setActivationError('technicianNotAuthorized');
      resetOtpFlow();
      return;
    }
    if (data?.status === 'ACTIVATION_NOT_FOUND') {
      setActivationError('activationNotFound');
      resetOtpFlow();
      return;
    }
    if (data?.status === 'OTP_INVALID') {
      const detail = data.remaining_attempts ? `(${data.remaining_attempts} attempts left)` : '';
      setActivationError('otpInvalid', detail);
      setOtpFlow(true);
      if (elements.verifyOtpButton) elements.verifyOtpButton.disabled = false;
      return;
    }
    if (data?.status === 'OTP_EXPIRED') {
      setActivationError('otpExpired');
      resetOtpFlow();
      return;
    }
    if (data?.status === 'OTP_LOCKED') {
      setActivationError('otpLocked');
      resetOtpFlow();
      return;
    }
    if (data?.status === 'SIMULATION_COMPLETED' && data?.final_state === 'JOB_CLOSED') {
      activationState.step = 'closed';
      updateActivationTimeline('closed');
      elements.activationResult.textContent = getTranslation('activationSuccessDetailed');
      const cabinet = elements.cabinetInput?.value || 'CAB-A';
      const ontSerial = elements.ontSerialInput?.value.trim() || 'ONT';
      startActivationAnimation(cabinet, ontSerial);
      return;
    }
    setActivationError('activationCheckInput');
    setOtpFlow(true);
    if (elements.verifyOtpButton) elements.verifyOtpButton.disabled = false;
  } catch (error) {
    setActivationError('activationCheckInput');
    setOtpFlow(true);
    if (elements.verifyOtpButton) elements.verifyOtpButton.disabled = false;
  }
}

function startActivationAnimation(cabinet, ontSerial) {
  const line = cabinet === 'CAB-B' ? 'FS-LINE-B' : cabinet === 'CAB-C' ? 'FS-LINE-C' : 'FS-LINE-A';
  const splitter = elements.splitterInput?.value.trim() || getSplitterForCabinet(cabinet);
  const roe = elements.roeInput?.value.trim() || 'ROE';
  const path = ['CENTRAL-01', cabinet, line, splitter, roe, ontSerial || 'ONT'];
  updateTopology(path);
  activationState.path = path;
  if (elements.llmStatusText) {
    elements.llmStatusText.textContent = 'EVENT → OBSERVE → LLM ANALYSIS → TOPOLOGY PROPOSAL → DETERMINISTIC VALIDATION → SIMULATED PROVISIONING → JOB_CLOSED';
  }
}

function bindForm() {
  if (!elements.activationForm) return;
  elements.activationForm.addEventListener('submit', (event) => {
    event.preventDefault();
  });
  if (elements.cabinetInput) {
    elements.cabinetInput.addEventListener('change', () => {
      const cabinet = elements.cabinetInput.value;
      activationState.cabinet = cabinet;
      if (elements.splitterInput) {
        elements.splitterInput.value = getSplitterForCabinet(cabinet);
      }
    });
  }
  if (elements.splitterInput && elements.cabinetInput) {
    elements.splitterInput.value = getSplitterForCabinet(elements.cabinetInput.value || 'CAB-A');
  }
  resetOtpFlow();
}

function bindActivationButtons() {
  const send = document.getElementById('sendOtpButton');
  const verify = document.getElementById('verifyOtpButton');

  if (send) {
    send.onclick = (event) => {
      event.preventDefault();
      startActivation();
    };
  }

  if (verify) {
    verify.onclick = (event) => {
      event.preventDefault();
      verifyActivation();
    };
  }
}

function initialize() {
  bindActivationButtons();
  initializeLanguage();
  bindForm();
  setupBackgroundCanvas();
  fetchLiveData();
  fetchOpenstackData();
  animationLoop();
}

function animateTopology() {
  if (!elements.topologyCanvas) return;
  const canvas = elements.topologyCanvas;
  const ctx = canvas.getContext('2d');
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  canvas.width = rect.width * dpr;
  canvas.height = rect.height * dpr;
  ctx.scale(dpr, dpr);
  ctx.clearRect(0, 0, rect.width, rect.height);

  const path = topologyState.path || [];
  if (!path.length) {
    ctx.fillStyle = 'rgba(255,255,255,.12)';
    ctx.font = '16px Inter';
    ctx.fillText('Topology data unavailable.', 20, 40);
    return;
  }

  const coords = path.map((_, index) => ({
    x: 80 + index * ((rect.width - 120) / Math.max(path.length - 1, 1)),
    y: rect.height / 2
  }));

  ctx.strokeStyle = 'rgba(76,201,255,.32)';
  ctx.lineWidth = 2;
  ctx.beginPath();
  coords.forEach((point, index) => {
    if (index === 0) ctx.moveTo(point.x, point.y);
    else ctx.lineTo(point.x, point.y);
  });
  ctx.stroke();

  let pulse = (Date.now() / 1200) % 1;
  coords.forEach((point, index) => {
    const label = path[index];
    const type = mapTopologyNode(label);
    const radius = 28;
    ctx.fillStyle = 'rgba(8,20,46,.9)';
    ctx.strokeStyle = 'rgba(76,201,255,.45)';
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.arc(point.x, point.y, radius, 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();

    ctx.fillStyle = 'rgba(255,255,255,.9)';
    ctx.font = '600 12px Inter';
    ctx.textAlign = 'center';
    ctx.fillText(type, point.x, point.y - 8);
    ctx.font = '500 11px Inter';
    ctx.fillText(label, point.x, point.y + 14);

    const progress = (pulse + index * 0.08) % 1;
    if (progress > 0.05 && progress < 0.95) {
      ctx.beginPath();
      ctx.arc(point.x, point.y, radius + 6 * progress, 0, Math.PI * 2);
      ctx.strokeStyle = `rgba(76,201,255,${0.25 * (1 - progress)})`;
      ctx.lineWidth = 2;
      ctx.stroke();
    }
  });
}

function updateTopology(path) {
  topologyState.path = path;
}

function animationLoop() {
  if (topologyAnimationId) cancelAnimationFrame(topologyAnimationId);
  topologyAnimationId = rAF(animateTopology);
}

document.addEventListener('DOMContentLoaded', initialize);


function setupBackgroundCanvas() {
  if (!elements.backgroundCanvas) return;
  const canvas = elements.backgroundCanvas;
  const ctx = canvas.getContext('2d');
  const points = Array.from({ length: 45 }, () => ({
    x: Math.random(),
    y: Math.random(),
    r: 1 + Math.random() * 1.8,
    vx: (Math.random() - .5) * 0.0004,
    vy: (Math.random() - .5) * 0.0004,
  }));
  const prefersReduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  function resizeCanvas() {
    const rect = canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }
  function draw() {
    if (prefersReduced) return;
    const rect = canvas.getBoundingClientRect();
    ctx.clearRect(0, 0, rect.width, rect.height);
    points.forEach((point, index) => {
      point.x += point.vx;
      point.y += point.vy;
      if (point.x < 0 || point.x > 1) point.vx *= -1;
      if (point.y < 0 || point.y > 1) point.vy *= -1;
      const px = point.x * rect.width;
      const py = point.y * rect.height;
      ctx.beginPath();
      ctx.fillStyle = 'rgba(76,201,255,.12)';
      ctx.arc(px, py, point.r, 0, Math.PI * 2);
      ctx.fill();
      for (let j = index + 1; j < points.length; j += 7) {
        const other = points[j];
        const dx = other.x - point.x;
        const dy = other.y - point.y;
        const dist = Math.hypot(dx, dy);
        if (dist < 0.24) {
          ctx.strokeStyle = `rgba(76,201,255,${0.08 * (1 - dist / 0.24)})`;
          ctx.lineWidth = 1;
          ctx.beginPath();
          ctx.moveTo(px, py);
          ctx.lineTo(other.x * rect.width, other.y * rect.height);
          ctx.stroke();
        }
      }
    });
    bgAnimationId = rAF(draw);
  }
  resizeCanvas();
  window.addEventListener('resize', resizeCanvas);
  if (!prefersReduced) draw();
}

function renderTopology(path) {
  updateTopology(path);
}

function setDemoData(data) {
  elements.healthStatus.textContent = data.status || 'unknown';
  elements.projectName.textContent = data.project || 'unknown';
  elements.projectState.textContent = data.status || 'unknown';
  elements.techAuth.textContent = data.technician?.authenticated ? 'YES' : 'NO';
  elements.ontDetected.textContent = data.ont?.detected ? 'YES' : 'NO';
  elements.ontSerial.textContent = data.ont?.serial || 'unknown';
  elements.llmProposal.textContent = data.llm?.role ? `Interpreting ${data.llm.role}` : 'pending';
  elements.backendValidation.textContent = data.backend?.validation || 'pending';
  elements.provisioningMode.textContent = data.provisioning?.mode || 'pending';
  elements.finalState.textContent = data.final_state || 'pending';
  elements.simulationStatus.textContent = data.simulation ? 'SIMULATION' : 'LIVE';
  elements.simulationStatus.className = 'status-text badge badge-simulation';
  elements.simulationDesc.textContent = data.simulation ? 'Laboratory simulation in progress' : 'Live status feed';
  renderTopology(data.path || []);
  updateLLMSteps(data);
}

async function fetchLiveData() {
  try {
    const healthRes = await fetch(`${apiBase}/health`);
    const demoRes = await fetch(`${apiBase}/lab/demo`);
    if (!healthRes.ok || !demoRes.ok) {
      setStatus(false);
      return;
    }
    const healthData = await healthRes.json();
    const demoData = await demoRes.json();
    setStatus(true);
    setDemoData(demoData);
  } catch (error) {
    setStatus(false);
  }
}

async function fetchOpenstackData() {
  try {
    const openstackRes = await fetch(`${apiBase}/lab/openstack`);
    if (!openstackRes.ok) {
      return setOpenstackOffline();
    }
    const openstackData = await openstackRes.json();
    setOpenstackData(openstackData);
  } catch (error) {
    setOpenstackOffline();
  }
}

